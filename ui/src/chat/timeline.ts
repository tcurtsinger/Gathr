import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Message } from '../api';
import { askSizes } from './asks';
import {
  block,
  eventsOf,
  failed,
  heard,
  kept,
  personBlock,
  questionBlock,
  saidBlock,
  talkBlock,
  versionBlock,
} from './blocks';
import { fold } from './fold';
import { type Group, givenBy, grouped, hasEnding, lastBlocks } from './groups';
import { askOf, foldOf, newest, stateOf, type Thread, threadsOf, timeOf } from './mains';
import {
  briefOf,
  type Cache,
  type Custom,
  type Place,
  type SendFailure,
  type Spot,
  type Timeline,
  type View,
} from './messages';
import { place as inOrder } from './order';
import { heardTurns, questions } from './questions';
import type { ChatState } from './state';
import { team } from './team';
import { talks } from './threads';
import { topicsOf } from './topics';

/** The chat drawn: see `toThread`. */
export function drawn(
  state: ChatState,
  names: Map<string, string>,
  unsent: SendFailure[],
  cache: Cache,
  view: View | undefined,
): Timeline {
  const crew = team(state, names);
  const saved = grouped(state);
  const { extra, hosts } = questions(state, saved);
  const groups = inOrder(extra.length ? [...saved, ...extra] : saved, state);
  const heardIn = heardTurns(state.tasks);
  const gave = givenBy(state, crew);
  const sizes = askSizes(state, crew);
  const lastOf = lastBlocks(groups, state);
  const numbered = new Map(state.tasks.map((t) => [t.number, t]));
  const { quiet, spoken } = talks(groups, state, crew, hasEnding);
  const worked = new Map([...quiet].map(([work, to]) => [to, work]));

  /** Who a reply block is by and the task it is on: a block that follows its speaker's isn't named again. */
  const heading = (g: Group, topic?: string) => {
    const turn = g.turn ? state.turns[g.turn] : undefined;
    // Only a member or the reviewer is headed by its task; the lead always by its role. In its own task's thread the
    // task's message says it, and its state: a reply there names no task.
    const led = !crew.role(turn?.agent) || crew.role(turn?.agent) === 'supervisor';
    const own = turn?.task != null && topic === `t:${turn.task}`;
    const on = led || own || turn?.task == null ? undefined : numbered.get(turn.task)?.what;
    const last = turn?.task != null && lastOf.get(turn.task) === g;
    return { on, last, who: `${turn?.agent}|${on}` };
  };
  /** One block as an item, or none when it draws nothing; `speaker` is whoever was named last in its run. */
  const draw = (g: Group, place: Place, run: { speaker?: string }, ticked?: boolean) => {
    if (quiet.has(g)) return null;
    const head = (who?: string) => {
      const named = who === undefined || who !== run.speaker;
      run.speaker = who;
      return named;
    };
    switch (g.kind) {
      case 'to': {
        head();
        const work = worked.get(g);
        const turn = work?.turn ? state.turns[work.turn] : undefined;
        const running = !!work?.last && turn?.status === 'running';
        const did = work && fold(work.msgs, '', running, gave, work.bound ? turn?.created : undefined).work;
        return talkBlock(cache, g, state, crew, { spoken, work: did, place });
      }
      case 'gathr': {
        const lines = heardIn.has(g.turn ?? '') ? [] : heard(g, state, crew);
        if (!lines.length) return null;
        head();
        return eventsOf(cache, g.key, lines, g.msgs[0].time, place);
      }
      case 'question':
        head();
        return questionBlock(cache, g, state, crew, place);
      case 'said':
        head();
        return saidBlock(cache, g, place);
      case 'person':
        head();
        return personBlock(cache, g.msgs[0], sizes.get(g.msgs[0].id), place);
      case 'version':
        // Named as the lead's reply blocks are: the lead's turn works on no task.
        return versionBlock(
          cache,
          g,
          state,
          crew,
          head(`${state.turns[g.turn ?? '']?.agent}|undefined`),
          place,
        );
      default: {
        const h = heading(g, place.topic);
        const value = block(cache, g, state, crew, gave, {
          ...h,
          head: head(h.who),
          asked: hosts.get(g),
          place,
          ticked,
        });
        // A block with nothing to show (Replies.tsx) names nobody.
        return value;
      }
    }
  };

  const items: ThreadMessageLike[] = [];
  const all: ThreadMessageLike[] = [];
  const spots = new Map<Group, { id: string; topic?: string }>();
  const handled = new Set<string>();
  const taskMains = new Map<number, string>();
  const isTeam = state.members.length > 1;
  const since = isTeam
    ? Date.parse(state.teamSince ?? '') || Number.NEGATIVE_INFINITY
    : Number.POSITIVE_INFINITY;
  const flat = isTeam ? groups.filter((g) => timeOf(g, state) < since) : groups;
  const run: { speaker?: string } = {};
  for (const g of flat) {
    const item = draw(g, {}, run);
    if (!item) continue;
    items.push(item);
    all.push(item);
    spots.set(g, { id: item.id as string });
  }

  if (isTeam) {
    const before = new Set(flat);
    const rest = flat.length ? groups.filter((g) => !before.has(g)) : groups;
    const topics = topicsOf(rest, state, spoken, hosts);
    const threads = threadsOf(rest, topics, state);
    // Each ask's newest result, by its topic: the ask's Done leads there, and the result quotes the ask.
    const results = new Map<string, string>();
    for (const t of threads) {
      const turn = t.topic.startsWith('r:') ? t.topic.slice(2) : undefined;
      const ask = turn && askOf(turn, state, topics, rest);
      if (ask) results.set(ask, t.topic);
    }
    const closes = new Map([...results].map(([ask, topic]) => [topic, ask]));
    for (const t of threads) drawThread(t, results, closes.get(t.topic));
  }

  // A send that opened a turn fails that turn too, and the turn's notice already says so.
  const told = new Set(Object.values(state.turns).map((t) => t.ref));
  for (const f of unsent) {
    if (f.failure.ref && told.has(f.failure.ref)) continue;
    items.push(
      kept(cache, `f-${f.id}`, `${f.text}|${f.failure.error}|${f.failure.ref}|${f.failure.why}`, () =>
        failed(f),
      ),
    );
  }
  return { items, all, find, handled };

  function drawThread(t: Thread, results: Map<string, string>, ask: string | undefined) {
    const { state: word, handled: settled } = stateOf(t, state, results);
    const quote = ask && state.asks.find((a) => a.id === ask)?.title;
    const mainPlace: Place = { topic: t.topic, state: word, ...(ask && quote && { result: { ask, quote } }) };
    const runs: { speaker?: string } = {};
    let main: ThreadMessageLike | null = null;
    let mainGroup = t.main;
    const replies = [...t.replies];
    if (t.task) {
      main = taskBlock(t, mainPlace);
      taskMains.set(t.task.number, main.id as string);
    } else {
      main = mainGroup ? draw(mainGroup, mainPlace, runs, !!ask) : null;
      // Its first block drew nothing (Gathr's words that opened a hear-back): the first that does is the message.
      while (!main && replies.length) {
        mainGroup = replies.shift();
        main = mainGroup ? draw(mainGroup, mainPlace, runs, !!ask) : null;
      }
    }
    if (!main) return;
    items.push(main);
    all.push(main);
    if (mainGroup) spots.set(mainGroup, { id: main.id as string });
    const reply: Place = { topic: t.topic, reply: true };
    const drawnReplies: { g: Group; item: ThreadMessageLike }[] = [];
    const run: { speaker?: string } = {};
    for (const g of replies) {
      const item = draw(g, reply, run);
      if (item) drawnReplies.push({ g, item });
    }
    const replyItems = drawnReplies.map((r) => r.item);
    // A turn that ended needing the person (Retry, Continue, Sign in) keeps its thread from being handled.
    const done = settled && ![main, ...replyItems].some(needsYou);
    if (done) handled.add(t.topic);
    const open = view ? (view.open.get(t.topic) ?? !(view.start ?? handled).has(t.topic)) : !done;
    const line = foldOf(t.topic, main, replyItems.length, replyItems.length > 0 && open);
    if (!line) return;
    all.push(...replyItems);
    items.push(
      kept(cache, `fold-${t.topic}`, JSON.stringify(line), () => ({
        id: `fold-${t.topic}`,
        role: 'assistant',
        content: [],
        metadata: { custom: { fold: line } satisfies Custom },
      })),
    );
    const { shown, hid } = open ? newest(replyItems, !!view?.earlier.has(t.topic)) : { shown: [], hid: 0 };
    if (hid) {
      const earlier = { topic: t.topic, n: hid };
      items.push(
        kept(cache, `earlier-${t.topic}`, JSON.stringify(earlier), () => ({
          id: `earlier-${t.topic}`,
          role: 'assistant',
          content: [],
          metadata: { custom: { earlier } satisfies Custom },
        })),
      );
    }
    items.push(...shown);
    const showing = new Set(shown);
    for (const { g, item } of drawnReplies)
      spots.set(g, { id: item.id as string, ...(!showing.has(item) && { topic: t.topic }) });
  }

  function needsYou(item: ThreadMessageLike) {
    const own = (item.metadata?.custom ?? {}) as Custom;
    return !!own.ending?.act && own.ending.act !== 'reference';
  }

  function taskBlock(t: Thread, place: Place) {
    const task = t.task as NonNullable<Thread['task']>;
    const giver = task.giver ?? undefined;
    const custom: Custom = {
      ...place,
      brief: briefOf(task, crew.name(task.owner)),
      by: giver,
      agent: giver ? (crew.name(giver) ?? 'An agent') : 'You',
      role: crew.role(giver),
      time: new Date(task.created_us / 1000).toISOString(),
      head: true,
    };
    return kept(cache, `task-${task.number}`, JSON.stringify(custom), () => ({
      id: `task-${task.number}`,
      role: 'assistant',
      content: [],
      metadata: { custom },
    }));
  }

  function find(message: string): Spot | undefined {
    const hit = state.messages.find((x) => x.id === message);
    // Gathr's copy of an agent's message shows as the message it carries.
    const m = (hit?.copy_of && state.messages.find((x) => x.id === hit.copy_of)) || hit;
    if (!m) return undefined;
    const own = groups.find((g) => g.msgs.includes(m));
    const at = own && spots.get(own);
    if (at && own && m.kind === 'text' && own.kind !== 'gathr') {
      const words = (x: Message) => (x.to ? (spoken.get(x.id)?.text ?? x.body) : x.body);
      const before = own.msgs.slice(0, own.msgs.indexOf(m));
      const nth = before.filter((x) => x.kind === 'text' && words(x) === words(m)).length;
      return { ...at, text: words(m) as string, nth };
    }
    if (at) return at;
    // A task's brief isn't drawn: the task's own message stands for it, else its turn's block, or the giver's until
    // that turn starts.
    const task = state.turns[m.turn ?? '']?.task;
    const main = task == null ? undefined : taskMains.get(task);
    if (main) return { id: main };
    const gave = task == null ? undefined : state.tasks.find((t) => t.number === task)?.given_in;
    const blocks = groups.filter((g) => g.kind === 'reply' && spots.has(g));
    const stand = [m.turn, gave].flatMap((turn) => blocks.find((g) => !!turn && g.turn === turn) ?? []);
    return stand.length ? spots.get(stand[0]) : undefined;
  }
}
