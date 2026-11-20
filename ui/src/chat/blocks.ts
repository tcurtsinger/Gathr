import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Message, ThinkingBody, ToolBody, Turn } from '../api';
import { agreedCheck, planLine, versionOf, versionWord } from '../plans/plans';
import { type AskSize, changed, fixed } from './asks';
import { ending } from './ends';
import { type Chip, duration, fold, type Given, type Work } from './fold';
import type { Group } from './groups';
import { blockLike, likeOpened, PERSON, saidLike, versionLike } from './likes';
import type { Cache, Custom, EventLine, Place, SendFailure } from './messages';
import type { ChatState } from './state';
import { type Team, titleOf } from './team';
import type { Said, Spoken } from './threads';

const engineWait = (t: Turn) => t.held === 'claude' || t.held === 'codex';

/** A member's check of the lead's answer (0080). */
const isCheck = (t: Turn | undefined) => t?.opened === 'check';

/** Who it is by and the task it is on, and its place in the chat: a main message or a reply in a thread. */
export interface Heading {
  on?: string;
  /** Its task's last block: the agent's panel ticks its task here once done. */
  last: boolean;
  head: boolean;
  asked?: number[];
  place: Place;
  /** It has a tick on the conversation map: an ask's result. */
  ticked?: boolean;
  /** Gathr's words that opened its turn, quoted at the top of its bubble (0078). */
  opened?: string;
}

/**
 * A block of an agent's turn: its words where it wrote them, each run of steps between them one line (the owner's call
 * of 2026-11-19), a plan version it wrote as its chip, a plan check's findings as the chip that ends it, and Gathr's
 * words that opened the turn as its quote (0078). One that didn't change is the same object as on the draw before
 * (`cache`), so the thread can tell which blocks to draw again.
 */
export function block(
  cache: Cache,
  group: Group,
  state: ChatState,
  crew: Team,
  gave: Map<string, Given>,
  { on, last: lastOfTask, head, asked, place, ticked, opened }: Heading,
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
  const task = state.tasks.find((t) => t.number === turn?.task);
  const end = endingOf(group, state);
  const ended = end ? `${turn?.status}|${turn?.ref}|${end.act}` : '';
  // A tick on the map (0039): in a chat of one, the agent's main messages, the last block of each of its turns once it
  // has ended; in a team, its main messages say where each piece of work began instead.
  const solo = state.members.length <= 1;
  const over = !['queued', 'waiting_sign_in', 'running'].includes(turn?.status ?? '');
  const toPerson = solo && !place.reply && group.last && !!id && over && !crew.rootOf(id);
  const tick = ticked
    ? (group.msgs[0]?.time ?? turn?.created)
    : toPerson
      ? (group.msgs.findLast((m) => m.kind === 'text')?.time ?? turn?.created ?? '')
      : undefined;
  // A plan version it wrote is a chip in its place (0078). While it waits on the person, the bubble's like is the
  // version's, which approves it, and its thumbs-ups include the checker's (0062).
  const found = group.msgs.flatMap((m) => versionOf(state.plans, m.id) ?? []);
  const chips = new Map(
    found.map(({ plan, version }) => [
      version.message as string,
      { plan: plan.id, version: version.n, state: versionWord(plan, version.n) },
    ]),
  );
  const newest = found.at(-1);
  const waiting =
    newest && versionLike(state, newest.plan, newest.version.n, newest.version.message as string);
  // Only the last text's: the like sits under the bubble, after the block's final words, and an earlier text's
  // thumbs-up would read as given to those.
  const said = chips.size ? group.msgs.filter((m) => !chips.has(m.id)) : group.msgs;
  const last = said.at(-1);
  const like = waiting ?? blockLike(state, said, asked ?? [], !!partial);
  const all = new Set([
    ...(!partial && last && (last.kind === 'text' || like) ? agreedBy(state, crew, last.id) : []),
    ...found.flatMap(({ version }) => [
      ...agreedBy(state, crew, version.message as string),
      ...agreedCheck(version, state, crew),
    ]),
  ]);
  const agreed = all.has('You') ? ['You', ...[...all].filter((n) => n !== 'You')] : [...all];
  // In the agent's panel, a tick after the task once it is done; a blocked, stopped or held task is its owner's face's
  // mark, never a tick (0071).
  const done = !!on && lastOfTask && task?.state === 'done';
  // A plan check that left findings ends its checker's last bubble with them (0078). Only a plan check: a code
  // review's notes stay in the Tasks view and the fix's brief.
  const left = lastOfTask && task?.checks != null && task.report?.how === 'left' && !!task.notes?.length;
  const findings = left ? task.notes : undefined;
  // A member's check of an answer says whose under its name line (0080).
  const checked = isCheck(turn) && turn?.answers ? crew.name(state.turns[turn.answers]?.agent) : undefined;
  const seat = crew.seated(turn?.agent);
  const who = `${agent}|${role}|${seat}|${on}|${done}|${head}|${tick}|${agreed}|${JSON.stringify(like)}`;
  const answering = approvals.map((a) => `${a.request}:${a.answer?.allow}:${a.answer?.failure?.ref}`);
  // The last block's line works while its turn runs, so the turn's state is part of what it is made from.
  // A pick-up that carries the block on makes it that turn's (groups.ts): which turn it is counts too.
  const status = group.last ? `${id}:${turn?.status}` : '';
  // The block began when its first turn was made, whichever turn ends it.
  const made = group.last ? (state.turns[group.first ?? '']?.created ?? turn?.created) : undefined;
  const sig = `${parts}|${partial.length}|${status}|${made}|${ended}|${answering}|${who}|${asked}|${JSON.stringify(place)}|${
    opened
  }|${JSON.stringify([...chips])}|${JSON.stringify(findings)}|${checked}`;
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
    head,
    approvals,
    tick,
    agreed,
    asked,
    like,
  };
  if (end) custom.ending = end;
  if (opened) custom.opened = opened;
  if (findings) custom.findings = findings;
  if (checked) custom.checked = checked;
  const value = toReply(group, partial, group.last ? turn : undefined, custom, { gave, chips, made });
  cache.set(group.key, { sig, value });
  return value;
}

/** What the person can do about how a block's turn ended, on the turn's last block. */
function endingOf(group: Group, state: ChatState) {
  const id = group.turn;
  const turn = id ? state.turns[id] : undefined;
  if (!group.last || !id || !turn) return undefined;
  // The turn that answers a stopped one is its carry-on: once it runs or has ended, the work went on
  // there. One still waiting, or stopped (it may never have started), leaves Continue with the person.
  const went = ['running', 'done', 'failed'];
  const carried =
    turn.status === 'stopped' &&
    Object.values(state.turns).some((t) => t.answers === id && went.includes(t.status));
  const how = ending(turn, carried);
  return how?.act === 'retry' && likeOpened(state, id) ? { ...how, act: 'reference' as const } : how;
}

/**
 * A turn's block the chat draws nothing for: no words, no steps, its turn not running, and nothing the person can act
 * on (an approval, a question it asked, what to do about how its turn ended); a turn Gathr dropped before it ran.
 */
export function mute(group: Group, state: ChatState, asked: number[] | undefined): boolean {
  if (group.kind !== 'reply' || asked?.length) return false;
  const id = group.turn;
  // A check that only gave its thumbs-up shows as that thumbs-up on the answer (0080): its steps draw nothing.
  const steps = !isCheck(id ? state.turns[id] : undefined);
  if (group.msgs.some((m) => (m.kind === 'text' ? !!m.body : steps))) return false;
  if (group.last && id && (state.partial[id] || state.turns[id]?.status === 'running')) return false;
  if (group.last && id && state.approvals.some((a) => a.turn === id)) return false;
  return !endingOf(group, state);
}

function toReply(
  group: Group,
  partial: string,
  turn: Turn | undefined,
  custom: Custom,
  { gave, chips, made }: { gave: Map<string, Given>; chips: Map<string, Chip>; made?: string },
): ThreadMessageLike {
  const status = turn?.status;
  const began = group.bound ? made : undefined;
  const { work, words, pieces, line } = fold(group.msgs, partial, status === 'running', gave, began, chips);
  if (work) custom.work = work;
  if (pieces.length) custom.pieces = pieces;
  if (line) custom.line = line;
  // How long the turn took (0082): on its last block once it is done, from when it was made, as the working line counts.
  const took =
    status === 'done' && made && turn?.ended && duration(Date.parse(turn.ended) - Date.parse(made));
  if (took) custom.took = took;
  return {
    id: group.key,
    role: 'assistant',
    content: words.map((text) => ({ type: 'text', text })),
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
  // The open check's words to an agent (0080): its Checked line says it, and the last word follows what it answers.
  if (turn?.opened === 'check' || turn?.opened === 'last_word') return [];
  const fix = m.turn ? undefined : fixed(text);
  if (fix) return [fix];
  if (TOLD.test(text)) return [];
  // The service's brief (tasks/words.rs `brief`) opens with the task's number and its giver.
  const brief = turn?.task != null && text.startsWith(`Task #${turn.task} from `);
  return brief ? [] : [{ kind: 'note', text }];
}

/** Gathr's lines that opened a turn as the quote at the top of its bubble (0078): "Sent back" with the person's words. */
export function quoteOf(lines: EventLine[]): string {
  return lines
    .map((e) => {
      const said = [e.who, e.text].filter(Boolean).join(' ');
      return e.kind === 'version' && e.text === 'Sent back' && e.more ? `${said}: ${e.more}` : said;
    })
    .join('\n');
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
  { spoken, did, place }: { spoken: Map<string, Spoken>; did?: { work?: Work; line?: Work }; place: Place },
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
    ...(did?.work && { work: did.work }),
    ...(did?.line && { line: did.line }),
    ...(titleOf(crew.role(agent)) && { title: titleOf(crew.role(agent)) }),
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
