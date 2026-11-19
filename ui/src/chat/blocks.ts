import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Message, ThinkingBody, ToolBody, Turn } from '../api';
import { agreedCheck, planLine, versionOf } from '../plans/plans';
import { type AskSize, changed, fixed } from './asks';
import { ending } from './ends';
import { fold, type Given } from './fold';
import type { Group } from './groups';
import { blockLike, likeOpened, PERSON, saidLike, versionLike } from './likes';
import type { Cache, Custom, EventLine, Place, SendFailure } from './messages';
import type { ChatState } from './state';
import type { Team } from './team';
import type { Said, Spoken } from './threads';

const engineWait = (t: Turn) => t.held === 'claude' || t.held === 'codex';

/** Who it is by and the task it is on, and its place in the chat: a main message or a reply in a thread. */
export interface Heading {
  on?: string;
  /** Its task's last block: how the task ended is said here. */
  last: boolean;
  head: boolean;
  asked?: number[];
  place: Place;
  /** It has a tick on the conversation map: an ask's result. */
  ticked?: boolean;
}

/**
 * A block of an agent's turn: what it did folds into a row, its last text is the message (0037). One that didn't
 * change is the same object as on the draw before (`cache`), so the thread can tell which blocks to draw again.
 */
export function block(
  cache: Cache,
  group: Group,
  state: ChatState,
  crew: Team,
  gave: Map<string, Given>,
  { on, last: lastOfTask, head, asked, place, ticked }: Heading,
) {
  const id = group.turn;
  const turn = id ? state.turns[id] : undefined;
  const partial = group.last && id ? (state.partial[id] ?? '') : '';
  const approvals = group.last ? state.approvals.filter((a) => a.turn === id) : [];
  const agent = crew.name(turn?.agent);
  const role = crew.role(turn?.agent);
  const parts = group.msgs.map((m) => {
    if (m.kind === 'text') return m.id;
    const { status, duration_ms } = m.body as ToolBody | ThinkingBody;
    const made = gave.get(m.id);
    return `${m.id}:${status}:${duration_ms}:${made?.task.number}:${made?.task.what}:${made?.owner}`;
  });
  // The turn that answers a stopped one is its carry-on: once it runs or has ended, the work went on
  // there. One still waiting, or stopped (it may never have started), leaves Continue with the person.
  const went = ['running', 'done', 'failed'];
  const carried =
    turn?.status === 'stopped' &&
    Object.values(state.turns).some((t) => t.answers === id && went.includes(t.status));
  const task = state.tasks.find((t) => t.number === turn?.task);
  const how = group.last && turn ? ending(turn, task, carried) : undefined;
  const end =
    how?.act === 'retry' && id && likeOpened(state, id) ? { ...how, act: 'reference' as const } : how;
  const ended = end ? `${turn?.status}|${turn?.ref}|${end.word}|${end.act}` : '';
  // A tick on the map (0039): in a chat of one, the lead's reply to the person, the last block of each of its turns
  // once it has ended; in a team, its main messages say where each piece of work began instead.
  const lead = state.members.length > 1 ? role === 'supervisor' : true;
  const over = !['queued', 'waiting_sign_in', 'running'].includes(turn?.status ?? '');
  const toPerson = !place.topic && group.last && !!id && lead && over && !crew.rootOf(id);
  const tick = ticked
    ? (group.msgs[0]?.time ?? turn?.created)
    : toPerson
      ? (group.msgs.findLast((m) => m.kind === 'text')?.time ?? turn?.created ?? '')
      : undefined;
  // Only the shown text's: an earlier text folds into the row, and its thumbs-up would sit under words
  // the giver didn't agree with.
  const last = group.msgs.at(-1);
  const like = blockLike(state, group.msgs, asked ?? [], !!partial);
  const agreed = !partial && last && (last.kind === 'text' || like) ? agreedBy(state, crew, last.id) : [];
  // A blocked, stopped or held task is its owner's face's mark, never a tick (0071); a word of the turn's says enough.
  const shown = on && lastOfTask ? task?.state : undefined;
  const done = shown === 'done';
  const out = !end?.word && (shown === 'cancelled' || shown === 'failed') ? shown : undefined;
  const seat = crew.seated(turn?.agent);
  const who = `${agent}|${role}|${seat}|${on}|${done}|${out}|${head}|${tick}|${agreed}|${JSON.stringify(like)}`;
  const answering = approvals.map((a) => `${a.request}:${a.answer?.allow}:${a.answer?.failure?.ref}`);
  const sig = `${parts}|${partial.length}|${ended}|${answering}|${who}|${asked}|${JSON.stringify(place)}`;
  const hit = cache.get(group.key);
  if (hit?.sig === sig) return hit.value;
  const time = group.msgs[0]?.time ?? turn?.created;
  const custom: Custom = {
    ...place,
    agent,
    by: turn?.agent ?? undefined,
    role,
    seat,
    time,
    on,
    done,
    out,
    head,
    approvals,
    tick,
    agreed,
    asked,
    like,
  };
  if (end) custom.ending = end;
  const value = toReply(group, partial, group.last ? turn : undefined, custom, gave);
  cache.set(group.key, { sig, value });
  return value;
}

function toReply(
  group: Group,
  partial: string,
  turn: Turn | undefined,
  custom: Custom,
  gave: Map<string, Given>,
): ThreadMessageLike {
  const status = turn?.status;
  const began = group.bound ? turn?.created : undefined;
  const { work, text } = fold(group.msgs, partial, status === 'running', gave, began);
  if (work) custom.work = work;
  if (text) custom.text = true;
  return {
    id: group.key,
    role: 'assistant',
    content: text ? [{ type: 'text', text }] : [],
    status:
      status === 'running' || (status === 'queued' && !(turn && engineWait(turn)))
        ? { type: 'running' }
        : status === 'failed'
          ? { type: 'incomplete', reason: 'error' }
          : status === 'stopped'
            ? { type: 'incomplete', reason: 'cancelled' }
            : { type: 'complete', reason: 'stop' },
    metadata: { custom },
  };
}

/** Who gave a message a thumbs-up, by name: "You" first, then agents in the order given; one Gathr can't name is left out. */
export function agreedBy(state: ChatState, crew: Team, message: string): string[] {
  const on = state.reactions.filter((r) => r.message === message);
  const agents = on.flatMap((r) => (r.agent !== PERSON && crew.name(r.agent)) || []);
  return on.some((r) => r.agent === PERSON) ? ['You', ...agents] : agents;
}

/** A version of the lead's plan: the speaker, its chip and who gave it a thumbs-up; its words are the tab's. */
export function versionBlock(
  cache: Cache,
  group: Group,
  state: ChatState,
  crew: Team,
  head: boolean,
  place: Place,
): ThreadMessageLike {
  const m = group.msgs[0];
  const found = versionOf(state.plans, m.id);
  const agent = state.turns[group.turn ?? '']?.agent;
  const custom: Custom = {
    ...place,
    agent: crew.name(agent),
    by: agent ?? undefined,
    role: crew.role(agent),
    seat: crew.seated(agent),
    time: m.time,
    head,
    agreed: [
      ...new Set([...agreedBy(state, crew, m.id), ...(found ? agreedCheck(found.version, state, crew) : [])]),
    ],
    plan: found && { plan: found.plan.id, version: found.version.n, parts: found.version.parts },
    like: found && versionLike(state, found.plan, found.version.n, m.id),
  };
  return kept(cache, group.key, JSON.stringify(custom), () => ({
    id: group.key,
    role: 'assistant',
    content: [],
    metadata: { custom },
  }));
}

// The service's words that open a turn after the person changes a size (plan_wait.rs `told`): for the agent, never
// drawn. Gathr's own short line says what the change held or released (0073).
const RESIZED = 'The person made "';

// What a task's end told (tasks/words.rs): a hear-back's first line, and Gathr's line for a task that ended done (`DONE`,
// 0051). The agents read both; the person reads each end on its member's own last message.
const TOLD =
  /^(?:The (?:task|\d+ tasks) you gave (?:is|are) settled|Where the (?:task|\d+ tasks) you gave stands?|A task is done):\n/;

/**
 * What Gathr told an agent, as lines in its own words. None for the brief of the turn's own task (the member's block
 * is headed by the task), for what a size change tells the lead (0073), or for what a task's end told (`TOLD`).
 */
export function heard(group: Group, state: ChatState, crew: Team): EventLine[] {
  const m = group.msgs[0];
  if (state.caught.includes(m.id)) return [changed(m.body as string)];
  const plan = planLine(m, state, crew);
  if (plan !== undefined) return plan ? [plan] : [];
  const text = m.body as string;
  const turn = state.turns[m.turn ?? ''];
  if (turn && text.startsWith(RESIZED)) return [];
  const fix = m.turn ? undefined : fixed(text);
  if (fix) return [fix];
  if (TOLD.test(text)) return [];
  // The service's brief (tasks/words.rs `brief`) opens with the task's number and its giver.
  const brief = turn?.task != null && text.startsWith(`Task #${turn.task} from `);
  return brief ? [] : [{ kind: 'note', text }];
}

/**
 * A question with no block of its asker's to sit in: Gathr's own, as Gathr's message, or an agent's whose turn drew
 * none, as that agent's. Its words are the block's (Asked.tsx); this is who asks and when.
 */
export function questionBlock(
  cache: Cache,
  group: Group,
  state: ChatState,
  crew: Team,
  place: Place,
): ThreadMessageLike {
  const t = state.tasks.find((x) => x.number === group.task);
  const time = t && new Date(t.created_us / 1000).toISOString();
  const custom: Custom = t?.giver
    ? {
        ...place,
        agent: crew.name(t.giver),
        by: t.giver,
        role: crew.role(t.giver),
        time,
        head: true,
        asked: [t.number],
      }
    : { ...place, gathr: true, time, asked: [group.task as number] };
  return kept(cache, group.key, JSON.stringify(custom), () => ({
    id: group.key,
    role: t?.giver ? 'assistant' : 'system',
    content: t?.giver ? [] : [{ type: 'text', text: t?.what ?? '' }],
    metadata: { custom },
  }));
}

/** The person's answer or Discuss to a question, as their own message, when they sent it. */
export function saidBlock(cache: Cache, group: Group, place: Place): ThreadMessageLike {
  const text = group.text ?? '';
  return kept(cache, group.key, `${text}|${group.time}|${JSON.stringify(place)}`, () => ({
    id: group.key,
    role: 'user',
    content: [{ type: 'text', text }],
    metadata: { custom: { ...place, time: group.time } satisfies Custom },
  }));
}

/** The block made on an earlier draw, while what it's made from (`sig`) is the same. */
export function kept(
  cache: Cache,
  key: string,
  sig: string,
  make: () => ThreadMessageLike,
): ThreadMessageLike {
  const hit = cache.get(key);
  if (hit?.sig === sig) return hit.value;
  const value = make();
  cache.set(key, { sig, value });
  return value;
}

/** Gathr's lines, said by Gathr at its message's `time`. */
export function eventsOf(
  cache: Cache,
  key: string,
  events: EventLine[],
  time: string,
  place: Place,
): ThreadMessageLike {
  return kept(cache, key, `${time}|${JSON.stringify(events)}|${JSON.stringify(place)}`, () => ({
    id: key,
    role: 'system',
    content: [
      { type: 'text', text: events.map((e) => [e.who, e.text].filter(Boolean).join(' ')).join('\n') },
    ],
    metadata: { custom: { ...place, events, gathr: true, time } satisfies Custom },
  }));
}

/** The person's own message, with their ask's size (0061), the names they picked, and agents' thumbs-ups (0057 C5). */
export function personBlock(
  cache: Cache,
  m: Message,
  { size, agreed }: { size?: AskSize; agreed: string[] },
  place: Place,
) {
  const sig = `${m.body}|${JSON.stringify([size, m.picks, agreed, place])}`;
  return kept(cache, m.id, sig, () => ({
    id: m.id,
    role: 'user',
    content: [{ type: 'text', text: m.body as string }],
    metadata: {
      // A reply in a thread (0076) has no tick: the map marks the main line.
      custom: {
        ...place,
        tick: place.reply ? undefined : m.time,
        time: m.time,
        size,
        picks: m.picks ?? undefined,
        agreed,
      } satisfies Custom,
    },
  }));
}

/** A message between agents (0072), an item of its own: who wrote it to whom, and its words. */
export function talkBlock(
  cache: Cache,
  group: Group,
  state: ChatState,
  crew: Team,
  { spoken, work, place }: { spoken: Map<string, Spoken>; work?: Said['work']; place: Place },
): ThreadMessageLike {
  const m = group.msgs[0];
  const agent = state.turns[m.turn ?? '']?.agent ?? undefined;
  const { quote, mention, text } = spoken.get(m.id) ?? { text: m.body as string };
  const said: Said = {
    id: m.id,
    from: crew.name(agent),
    agent,
    by: state.members.some((x) => x.agent === agent) ? agent : undefined,
    to: mention,
    quote,
    time: m.time,
    agreed: agreedBy(state, crew, m.id),
    like: saidLike(state, m.id),
    ...(work && { work }),
  };
  return kept(cache, group.key, JSON.stringify([said, text, place]), () => ({
    id: group.key,
    role: 'assistant',
    content: [{ type: 'text', text }],
    metadata: { custom: { ...place, said, by: agent } satisfies Custom },
  }));
}

export function failed(f: SendFailure): ThreadMessageLike {
  const custom: Custom = { unsent: f, picks: f.picks };
  return { id: `f-${f.id}`, role: 'user', content: [{ type: 'text', text: f.text }], metadata: { custom } };
}
