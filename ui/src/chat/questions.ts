import type { Message, Task } from '../api';
import { answerWords } from '../tasks/model';
import type { Group } from './messages';
import type { ChatState } from './state';

/**
 * The person's questions in the timeline (0073). An agent's sits in the block of the turn that asked it; Gathr's own,
 * or one whose turn drew no block, is a block of its own where it was asked. Each Discuss and the answer are the
 * person's own messages where they were sent: none of these has a message of its own, so each takes the place after
 * the last saved message from before it, as the window met them live. `groups` are the chat's, in saved order.
 */
export function questions(
  state: ChatState,
  groups: Group[],
): { extra: Group[]; hosts: Map<Group, number[]> } {
  const asked = state.tasks.filter((t) => t.kind === 'person');
  const hosts = new Map<Group, number[]>();
  if (!asked.length) return { extra: [], hosts };
  const slot = slotter(state.messages);
  const extra: (Group & { us: number })[] = [];
  for (const t of asked) {
    const at = slot(t.created_us);
    const own = groups.filter((g) => g.kind === 'reply' && !!t.giver && g.turn === t.given_in);
    const host = own.findLast((g) => g.at <= at) ?? own[0];
    if (host) hosts.set(host, [...(hosts.get(host) ?? []), t.number]);
    else extra.push({ ...block(`q-${t.number}`, 'question', at), task: t.number, us: t.created_us });
    const said = (key: string, text: string, us: number | undefined) => {
      // Sent before answers kept their time: right under the question.
      const place = us === undefined ? at + 0.25 : slot(us);
      const time = us === undefined ? undefined : new Date(us / 1000).toISOString();
      extra.push({ ...block(key, 'said', place), task: t.number, text, time, us: us ?? t.created_us + 1 });
    };
    for (const [i, d] of (t.answer?.discussed ?? []).entries())
      said(`said-${t.number}-${i}`, d.text, d.at ?? heardAt(state.messages, d.turn));
    const words = answerWords(t);
    if (words.length)
      said(`said-${t.number}`, words.join('\n'), t.answer?.at ?? heardAt(state.messages, t.heard_in));
  }
  extra.sort((a, b) => a.at - b.at || a.us - b.us);
  return { extra, hosts };
}

const block = (key: string, kind: 'question' | 'said', at: number): Group => ({
  key,
  turn: null,
  msgs: [],
  kind,
  at,
  bound: false,
});

/**
 * Where something said at a time (µs) sits among the saved messages: after the last one from before it. A message's
 * time is to the millisecond, and what an answer starts is saved in the same one, so only an earlier one is before.
 */
function slotter(messages: Message[]) {
  const times = messages.map((m) => Date.parse(m.time));
  return (us: number) => {
    const ms = Math.floor(us / 1000);
    for (let i = times.length - 1; i >= 0; i--) if (times[i] < ms) return i + 0.5;
    return -0.5;
  };
}

/** When the turn that heard it began, in µs: Gathr's words that open it were saved with the answer. */
function heardAt(messages: Message[], turn: string | null | undefined): number | undefined {
  const opened = turn ? messages.find((m) => m.turn === turn && m.role === 'gathr') : undefined;
  return opened ? Date.parse(opened.time) * 1000 : undefined;
}

/** The turns whose opening Gathr's words only tell the asker what the person said: the person's message says it. */
export function heardTurns(tasks: Task[]): Set<string> {
  const turns = new Set<string>();
  for (const t of tasks) {
    if (t.kind !== 'person') continue;
    if (t.heard_in && t.state === 'done') turns.add(t.heard_in);
    for (const d of t.answer?.discussed ?? []) turns.add(d.turn);
  }
  return turns;
}
