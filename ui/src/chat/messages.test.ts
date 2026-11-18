// The chat's blocks (0037, 0075): where each sits, in a chat of one as one timeline and in a team in its main message's
// thread, what a turn folds into its row, who talks to whom, and which of Gathr's notes become its own messages.
import { expect, test } from 'vitest';
import { Failure, type Message, type ThinkingBody } from '../api';
import { duration } from './fold';
import {
  agents,
  alone,
  at,
  byId,
  custom,
  events,
  ids,
  round,
  said,
  task,
  text,
  tool,
  turn,
  view,
} from './madeUp';
import { toThread } from './messages';
import { apply, type ChatState } from './state';

test('a chat of one agent is one list in order: the person, then each turn where it began', () => {
  expect(ids(view(alone(round())).items)).toEqual([
    'ask-user',
    'a-ask',
    'mid-user',
    'a-mid',
    'direct-user',
    'a-direct',
    'a-rowan',
    'a-kit',
  ]);
});

test('a team chat is its main messages in time order, each with its thread; a member is headed by its task', () => {
  const { items, all } = view(round());
  // The tasks the lead handed out are main messages, at the time they were given; a done task's thread starts folded.
  expect(ids(items)).toEqual([
    'ask-user',
    'fold-m:ask-user',
    'task-1',
    'fold-t:1',
    'task-2',
    'fold-t:2',
    'a-kit',
    'mid-user',
    'fold-m:mid-user',
    'direct-user',
    'fold-m:direct-user',
  ]);
  expect(ids(all)).toEqual([
    'ask-user',
    'a-ask',
    'task-1',
    'a-rowan',
    'task-2',
    'a-kit',
    'mid-user',
    'a-mid',
    'direct-user',
    'a-direct',
  ]);
  // In its own task's thread a member's block names no task: the task's message says it. Gathr's message that gave it
  // isn't drawn.
  expect(custom(byId(all, 'a-rowan'))).toMatchObject({ agent: 'Rowan', role: 'member', reply: true });
  expect(custom(byId(all, 'a-rowan')).on).toBeUndefined();
  expect(custom(byId(all, 'a-ask')).on).toBeUndefined();
  expect(custom(byId(items, 'task-2'))).toMatchObject({
    brief: { what: 'Task 2', owner: 'Kit' },
    agent: 'Mira',
  });
});
test("replies that finish out of order keep their turns' order, as a reload shows them", () => {
  const state = round();
  // Rowan's turn started first; Kit's reply was saved first.
  const rowan = state.messages.filter((m) => m.turn === 'rowan');
  state.messages = [...state.messages.filter((m) => m.turn !== 'rowan'), ...rowan];
  expect(ids(view(alone(state)).items).slice(-2)).toEqual(['a-rowan', 'a-kit']);
});

test("two turns started live keep the service's order, whatever order their updates arrive in", () => {
  const state = round();
  const live = (id: string, agent: string, n: number, position: number, created: string) =>
    ({ type: 'turn', chat: 'c', turn: id, status: 'running', agent, task: n, position, created }) as const;
  // Lane's task turn started before Sol's, but Sol's update arrived first; neither has saved a word.
  const started = [
    live('sol', 'sol', 4, 7, '2026-11-08T09:05:01Z'),
    live('lane', 'lane', 3, 6, '2026-11-08T09:05:00Z'),
  ];
  let next = started.reduce(apply, state);
  next.tasks.push(task(3, 'lane', 'ask', { state: 'working' }), task(4, 'sol', 'ask', { state: 'working' }));
  next = { ...next, partial: { sol: 'Sol starts', lane: 'Lane starts' } };
  const { items } = view(next);
  expect(ids(items).slice(-2)).toEqual(['a-lane', 'a-sol']);
  expect(custom(byId(items, 'a-sol')).time).toBe('2026-11-08T09:05:01Z');
});

test('turns streaming at once never interleave, and a late reply lands in its own turn’s block', () => {
  const state = round();
  state.messages.push(
    tool('rowan', 'r1', 'Bash'),
    tool('kit', 'k1', 'Bash'),
    said('rowan', 'assistant', 'Rowan again'),
    said('kit', 'assistant', 'Kit again'),
    // The lead, whose turn began first, writes once more after its members.
    said('ask', 'assistant', 'Both are on it'),
  );
  const { items } = view(alone(state));
  expect(ids(items)).toEqual([
    'ask-user',
    'a-ask',
    'mid-user',
    'a-mid',
    'direct-user',
    'a-direct',
    'a-rowan',
    'a-kit',
  ]);
  expect(text(byId(items, 'a-ask'))).toBe('Both are on it');
  expect(text(byId(items, 'a-rowan'))).toBe('Rowan again');
  expect(text(byId(items, 'a-kit'))).toBe('Kit again');
});

test('a turn waiting to start stays where its message was sent, and what comes after it keeps its place', () => {
  const state = round();
  // Held by a pause: no place among the turns yet.
  state.turns.held = turn('held', 'mira', { status: 'queued', held: 'pause' });
  state.turns.late = { ...turn('late', 'lane'), position: 5 };
  state.messages.push(said('held', 'user', 'One more thing'), said('late', 'assistant', 'Lane, later'));
  expect(ids(view(alone(state)).items).slice(-3)).toEqual(['a-kit', 'held-user', 'a-late']);
});

test("a person's message sent into a running turn splits that turn's block where it was sent", () => {
  const state = round();
  state.turns.kit.status = 'running';
  state.messages.push(said('kit', 'user', '@Kit keep the shortcuts'), said('kit', 'assistant', 'Kept'));
  state.partial = { kit: 'Still checking' };
  const { items } = view(alone(state));
  expect(ids(items).slice(-3)).toEqual(['a-kit', 'kit-user', 'a-kit-1']);
  expect(text(byId(items, 'a-kit'))).toBe('Checked');
  // What it said before the stream is mid-work now; the stream is its message.
  expect(custom(items.at(-1)).work?.steps).toMatchObject([{ say: 'Kept' }]);
  expect(text(items.at(-1))).toBe('Still checking');
});

test('a finished turn folds what it did into one row, and its last text is the message', () => {
  const state = round();
  const t0 = Date.UTC(2026, 10, 8, 10, 0, 0);
  const iso = (s: number) => new Date(t0 + s * 1000).toISOString();
  state.turns.lead = { ...turn('lead', 'mira', { created: iso(0) }), position: 9 };
  state.tasks.push(
    task(7, 'kit', 'lead', { what: 'Open the settings PR' }),
    task(8, 'rowan', 'lead', { what: 'Bring the fix up to date' }),
  );
  state.messages.push(
    tool('lead', 't1', 'Read', { duration_ms: 400 }, iso(1)),
    tool('lead', 't2', 'Bash', { status: 'error', duration_ms: 27_000 }, iso(2)),
    { ...said('lead', 'assistant', 'Two tasks are stale.'), time: iso(40) },
    tool('lead', 't3', 'mcp__gathr__list_tasks', {}, iso(41)),
    tool('lead', 't4', 'Glob', {}, iso(42)),
    tool('lead', 't5', 'mcp__gathr__give_task', {}, iso(50)),
    tool('lead', 't6', 'mcp__gathr__give_task', { duration_ms: 9_000 }, iso(100)),
    { ...said('lead', 'assistant', 'Two things are left.'), time: iso(109) },
  );
  const block = byId(view(state).all, 'a-lead');
  expect(text(block)).toBe('Two things are left.');
  const work = custom(block).work;
  expect(work).toMatchObject({
    running: false,
    label: 'Worked for 1m 49s',
    notes: ['6 steps', 'gave 2 tasks'],
  });
  expect(work?.steps).toEqual([
    { id: 't1', label: 'Read a file', doing: 'Reading a file', state: 'ok' },
    { id: 't2', label: 'Ran a command', doing: 'Running a command', state: 'error' },
    { id: expect.any(String), say: 'Two tasks are stale.' },
    { id: 't3', label: 'Listed tasks', doing: 'Listing tasks', state: 'ok' },
    { id: 't4', label: 'Glob', doing: 'Glob', state: 'ok' },
    {
      id: 't5',
      label: 'Gave Kit a task',
      doing: 'Giving a task',
      detail: '#7 Open the settings PR',
      task: 7,
      state: 'ok',
    },
    {
      id: 't6',
      label: 'Gave Rowan a task',
      doing: 'Giving a task',
      detail: '#8 Bring the fix up to date',
      task: 8,
      state: 'ok',
    },
  ]);
});

test('a look-into task the turn gave reads so in its chip', () => {
  const state = round();
  state.turns.lead = { ...turn('lead', 'mira'), position: 9 };
  state.tasks.push(task(12, 'rowan', 'lead', { what: 'Survey tag libraries', look_into: true }));
  state.messages.push(
    tool('lead', 't1', 'mcp__gathr__give_task'),
    said('lead', 'assistant', 'Rowan looks first.'),
  );
  expect(custom(byId(view(state).all, 'a-lead')).work?.steps).toMatchObject([
    { label: 'Gave Rowan a task', detail: '#12 Look into: Survey tag libraries', task: 12 },
  ]);
});

test('a step shows what it ran as its chip, under the engine’s own words for it when it gave some (0041)', () => {
  const state = round();
  state.turns.lead = { ...turn('lead', 'mira'), position: 5 };
  const ran = (text: string, title?: string) => ({ input: { text, title } });
  state.messages.push(
    tool('lead', 's1', 'Bash', ran('git status --short', 'Show what changed')),
    tool('lead', 's2', 'Shell', ran('cargo clippy --workspace')),
    tool('lead', 's3', 'Read', ran('src/lib.rs')),
    tool('lead', 's4', 'Write', ran('notes/plan.md')),
    tool('lead', 's5', 'Grep', ran('fn main')),
    tool('lead', 's6', 'WebFetch', { ...ran('https://example.test/docs'), status: 'error' }),
    tool('lead', 's7', 'WebSearch', ran('posix shell quoting')),
    tool('lead', 's8', 'Agent', ran('Find the tests')),
    tool('lead', 's9', 'PowerShell', { ...ran('cargo nextest run'), status: 'running' }),
    // From before steps kept what they ran, and a tool that keeps nothing: the label alone, as it was.
    tool('lead', 's10', 'Bash'),
    tool('lead', 's11', 'mcp__gathr__list_tasks'),
    // With no input (an old message, a sub-agent's kind of tool, a search before Codex names it): words, not the tool's name.
    tool('lead', 's12', 'Grep'),
    tool('lead', 's13', 'WebFetch'),
    tool('lead', 's14', 'Agent'),
    tool('lead', 's15', 'WebSearch', { status: 'running' }),
  );
  state.turns.lead.status = 'running';
  const work = custom(byId(view(state).all, 'a-lead')).work;
  // What each step says now: a running one in the present (Tool call swaps between the two).
  const shown = work?.steps.map((s) =>
    'say' in s ? s.say : [s.state === 'running' ? s.doing : s.label, s.detail],
  );
  expect(shown).toEqual([
    ['Show what changed', 'git status --short'],
    ['Ran', 'cargo clippy --workspace'],
    ['Read', 'src/lib.rs'],
    ['Edited', 'notes/plan.md'],
    ['Searched', 'fn main'],
    ['Fetched', 'https://example.test/docs'],
    ['Searched the web', 'posix shell quoting'],
    ['Ran a sub-agent', 'Find the tests'],
    ['Running', 'cargo nextest run'],
    ['Ran a command', undefined],
    ['Listed tasks', undefined],
    ['Searched', undefined],
    ['Fetched', undefined],
    ['Ran a sub-agent', undefined],
    ['Searching the web', undefined],
  ]);
  // The row's own label while it runs is the last running step's, in the words it had: no chip in it.
  expect(work?.label).toBe('Searching the web');
});

test('a running turn’s row says what the agent is doing now; text streaming last is its message', () => {
  const state = round();
  state.turns.kit.status = 'running';
  state.messages.push(tool('kit', 'k1', 'Edit'), tool('kit', 'k2', 'Shell', { status: 'running' }));
  const running = custom(byId(view(state).all, 'a-kit')).work;
  expect(running).toMatchObject({ running: true, label: 'Running a command', notes: ['2 steps'] });
  expect(running?.steps[0]).toEqual({ id: expect.any(String), say: 'Checked' });
  expect(text(byId(view(state).all, 'a-kit'))).toBe('');

  // The command ended and it writes: the row stays, the words show as its message.
  state.messages[state.messages.length - 1] = tool('kit', 'k2', 'Shell');
  state.partial = { kit: 'All 360 pass' };
  const block = byId(view(state).all, 'a-kit');
  expect(custom(block).work).toMatchObject({ running: true, label: 'Working', notes: ['2 steps'] });
  expect(text(block)).toBe('All 360 pass');
});

test('a turn that only wrote once has no row', () => {
  const { all: items } = view(round());
  expect(custom(byId(items, 'a-rowan')).work).toBeUndefined();
  expect(text(byId(items, 'a-rowan'))).toBe('Cursor fixed');
});

test('a row’s times read as a person says them, and nothing under a second shows', () => {
  expect([400, 9_000, 109_000, 365_000, 3_900_000].map((ms) => duration(ms))).toEqual([
    '',
    '9s',
    '1m 49s',
    '6m 05s',
    '1h 5m',
  ]);
  // In whole minutes, for a line that is redrawn once a minute: never less than one.
  expect([400, 59_000, 109_000, 3_900_000].map((ms) => duration(ms, 'minute'))).toEqual([
    '1m',
    '1m',
    '1m',
    '1h 5m',
  ]);
});

test('messages between agents sit in the thread of what they are about, a reply after what it answers; Gathr’s copy stays hidden', () => {
  const state = round();
  // Kit asks Mira mid-turn, Mira answers from the turn Kit's message opened; later Mira writes to Rowan.
  const ask: Message = { ...said('kit', 'assistant', 'Is 30 seconds fine?'), id: 'ask-1', to: 'mira' };
  // The turn Kit's message opened is about what Kit's turn is about (the service follows the copy).
  state.turns.told = { ...turn('told', 'mira', { answers: 'kit', topic: 't:2' }), position: 5 };
  const copy: Message = {
    ...said('told', 'gathr', 'Kit wrote to you: Is 30 seconds fine?'),
    copy_of: 'ask-1',
  };
  const answer: Message = { ...said('told', 'assistant', 'Yes, 30 seconds.'), id: 'answer-1', to: 'kit' };
  state.messages.push(
    ask,
    copy,
    tool('told', 's1', 'mcp__gathr__message'),
    answer,
    said('told', 'assistant', 'Kit has its answer'),
  );
  const later: Message = { ...said('told', 'assistant', 'Rowan, rebase first'), id: 'later-1', to: 'rowan' };
  state.messages.push(later);
  const { all: items, find } = view(state);

  // Each is an item of its own in Kit's task's thread, in the order they were said.
  const order = ids(items);
  expect(order.slice(order.indexOf('task-2'), order.indexOf('mid-user'))).toEqual([
    'task-2',
    'a-kit',
    'ask-1',
    'a-told',
    'answer-1',
    'later-1',
  ]);
  expect(custom(byId(items, 'ask-1'))).toMatchObject({ reply: true, said: { from: 'Kit', to: 'Mira' } });
  expect(custom(byId(items, 'answer-1')).said).toMatchObject({ from: 'Mira', to: 'Kit' });
  expect(text(byId(items, 'answer-1'))).toBe('Yes, 30 seconds.');
  expect(custom(byId(items, 'later-1')).said).toMatchObject({ from: 'Mira', to: 'Rowan' });
  expect(text(byId(items, 'a-told'))).toBe('Kit has its answer');
  expect(custom(byId(items, 'a-told')).work?.steps).toMatchObject([{ label: 'Sent a message' }]);
  // A search hit on Gathr's copy opens at the message it carries, here behind the thread's earlier replies.
  expect(find(copy.id)).toEqual({ id: 'ask-1', text: 'Is 30 seconds fine?', nth: 0, topic: 't:2' });
  expect(find('answer-1')).toEqual({ id: 'answer-1', text: 'Yes, 30 seconds.', nth: 0 });
});

test('Gathr’s notes are its own messages, each with its time; what a task’s end told is not drawn, read or not', () => {
  const state = round();
  const ask = `m:${state.messages[0].id}`;
  state.turns.back = { ...turn('back', 'mira', { answers: 'ask', topic: ask }), position: 5 };
  // A wake nothing ties: a main message of its own, opened by Gathr's words.
  state.turns.woke = { ...turn('woke', 'mira', { topic: 'b:woke' }), position: 7 };
  state.messages.push(
    said(
      'back',
      'gathr',
      'The task you gave is settled:\n- #1 Rowan, Task 1: done: Approved at head 9a41c07\nRead.',
    ),
    said('back', 'assistant', 'Rowan is done'),
    // One that can't be read whole (task 7 is not the chat's) isn't drawn either.
    said(
      'back',
      'gathr',
      'The 2 tasks you gave are settled:\n- #1 Rowan, Task 1: done\n- #7 Sol, Task 7: done\nRead.',
    ),
    { ...said('none', 'gathr', 'A task is done:\n- #1 Rowan, Task 1: done'), turn: null, topic: 't:1' },
    said('woke', 'gathr', 'Your wait on CI on #12 ended'),
    said('woke', 'assistant', 'CI passed'),
  );
  const { all } = view(state);
  expect(events(all)).toEqual([{ kind: 'note', text: 'Your wait on CI on #12 ended' }]);
  // It is the wake's main message, and the reply it woke is in its thread.
  expect(ids(all).slice(-2)).toEqual(['woke-gathr', 'a-woke']);
  expect(custom(all.at(-1)).reply).toBe(true);
  const woke = state.messages.at(-2) as Message;
  expect(custom(all.at(-2)).time).toBe(woke.time);
  expect(all.filter((m) => custom(m).events).every((m) => m.role === 'system')).toBe(true);
});
test('a speaker is named when it changes, not on each of its blocks in a row', () => {
  const state = round();
  state.turns.again = { ...turn('again', 'kit', { task: 2, topic: 't:2' }), position: 5 };
  state.turns.other = { ...turn('other', 'kit', { task: 3, topic: 't:3' }), position: 6 };
  state.tasks.push(task(3, 'kit', 'ask'));
  state.messages.push(said('again', 'assistant', 'One more thing'), said('other', 'assistant', 'Next task'));
  const { all: items } = view(state);
  const heads = Object.fromEntries(items.map((m) => [m.id, custom(m).head]));
  expect(heads).toMatchObject({
    'a-ask': true,
    'a-rowan': true,
    'a-kit': true,
    'a-again': false,
    // The same agent on another task, in that task's thread, says which.
    'a-other': true,
  });
});

test('a search hit opens at its message: the text in its block, or the block for what isn’t drawn', () => {
  const state = round();
  state.messages.push(
    said('rowan', 'assistant', 'Same words'),
    tool('rowan', 'r1', 'Bash'),
    said('rowan', 'assistant', 'Same words'),
  );
  const { find } = view(state);
  const [first, , last] = state.messages.slice(-3);
  // A mid-work text is in the row; the same words as the message are the second passage. The task is done: its thread
  // is folded, so the hit names it, to open first.
  expect(find(first.id)).toEqual({ id: 'a-rowan', text: 'Same words', nth: 0, topic: 't:1' });
  expect(find(last.id)).toEqual({ id: 'a-rowan', text: 'Same words', nth: 1, topic: 't:1' });
  // Gathr's message that gave the task isn't drawn: the task's own message stands for it.
  const gave = state.messages.find((m) => m.role === 'gathr' && m.turn === 'rowan') as Message;
  expect(find(gave.id)).toEqual({ id: 'task-1' });
  expect(find('gone')).toBeUndefined();
});

const lines = (state: ChatState) =>
  events(view(state).all).map((e) => [e.who, e.text, e.more].filter(Boolean));
/** Mira hears back in a turn that answers `ask`, with these words from Gathr; it is about the person's ask. */
function heardBack(state: ChatState, id: string, body: string, position: number) {
  const topic = `m:${state.messages[0].id}`;
  state.turns[id] = { ...turn(id, 'mira', { answers: 'ask', topic }), position };
  state.messages.push(said(id, 'gathr', body), said(id, 'assistant', 'Noted'));
}

test('what Gathr says that is not a hear-back is drawn in its own words, never replaced or dropped', () => {
  const state = round();
  state.tasks.push(task(54, 'kit', 'ask', { what: 'Open the PR' }));
  // A wait's result names a number that is also a task the answered turn gave.
  const woke = 'Your wait ended: CI on #54 at abc1234 finished: passed. You planned to do this: merge.';
  heardBack(state, 'woke', woke, 5);
  // A member on its task is told its turn was interrupted; only the task's own brief stays hidden.
  const usage = "The account's usage limit interrupted your turn. Check what is done first.";
  state.turns.again = { ...turn('again', 'kit', { task: 2, answers: 'ask', topic: 't:2' }), position: 6 };
  state.messages.push(said('again', 'gathr', usage), said('again', 'assistant', 'Carrying on'));
  state.turns.redo = { ...turn('redo', 'kit', { task: 2, answers: 'ask', topic: 't:2' }), position: 7 };
  state.messages.push(said('redo', 'gathr', 'Task #2 from Mira: Task 2\nContinue from here: half done.'));
  state.messages.push(said('redo', 'assistant', 'Done now'));
  expect(lines(state)).toEqual([[woke], [usage]]);
  // Each in its thread, where it happened: the wake's words before the reply they woke.
  const all = ids(view(state).all);
  expect(all.slice(all.indexOf('woke-gathr'), all.indexOf('woke-gathr') + 2)).toEqual([
    'woke-gathr',
    'a-woke',
  ]);
  expect(all.slice(all.indexOf('again-gathr'), all.indexOf('again-gathr') + 3)).toEqual([
    'again-gathr',
    'a-again',
    'a-redo',
  ]);
});

test('a hit on a task brief opens at the task’s message; in a chat of one, at the turn that gave the task', () => {
  const state = round();
  state.turns.wait = turn('wait', 'lane', { task: 3, status: 'queued', held: 'pause', topic: 't:3' });
  state.tasks.push(task(3, 'lane', 'ask', { state: 'ready' }));
  const brief = said('wait', 'gathr', 'Task #3 from Mira: Task 3');
  state.messages.push(brief);
  expect(view(state).find(brief.id)).toEqual({ id: 'task-3' });
  expect(view(alone(state)).find(brief.id)).toEqual({ id: 'a-ask' });
});

test('a turn that wrote no words and only sent a message has no block: its steps are on the message, where it began', () => {
  const state = round();
  // Kit writes to Rowan; Lane works and then writes to Sol, with nothing else to say.
  const first: Message = { ...said('kit', 'assistant', 'Kit to Rowan'), id: 'first-1', to: 'rowan' };
  state.turns.other = { ...turn('other', 'lane'), position: 5 };
  const second: Message = { ...said('other', 'assistant', 'Lane to Sol'), id: 'second-1', to: 'sol' };
  state.messages.push(first, tool('other', 'o1', 'mcp__gathr__message'), second);
  const { all } = view(state);
  // Kit's is in its task's thread; Lane's turn is tied to nothing, so its message is a main message of its own.
  expect(custom(byId(all, 'first-1'))).toMatchObject({ reply: true, topic: 't:2' });
  expect(custom(byId(all, 'second-1'))).toMatchObject({ topic: 'b:other' });
  expect(custom(byId(all, 'second-1')).reply).toBeUndefined();
  expect(ids(all)).not.toContain('a-other');
  expect(custom(byId(all, 'second-1')).said?.work?.steps).toMatchObject([{ label: 'Sent a message' }]);
});

test('a block that did not change is the same object on the next draw; a changed one is new', () => {
  const state = round();
  state.messages.push(tool('kit', 'k1', 'Bash', { status: 'running' }));
  const cache = new Map();
  const before = toThread(state, agents, [], cache).all;
  const after = toThread({ ...state, partial: { rowan: 'More' } }, agents, [], cache).all;
  const same = before.map((m, i) => m === after[i]);
  // Only Rowan's block, now streaming, was made again: the person's messages are kept too.
  expect(same).toEqual(ids(before).map((id) => id !== 'a-rowan'));

  const ended = { ...state, messages: [...state.messages.slice(0, -1), tool('kit', 'k1', 'Bash')] };
  const next = toThread(ended, agents, [], cache).all;
  const kit = (all: typeof next) => byId(all, 'a-kit');
  expect(kit(next)).not.toBe(kit(before));
  expect(custom(kit(next)).work?.steps.at(-1)).toMatchObject({ state: 'ok' });
});

test('a turn with only its ending has its block after the message that began it; one still waiting has none', () => {
  const state = round();
  state.turns.bad = { ...turn('bad', 'mira', { status: 'failed' }), position: 5 };
  state.turns.queued = turn('queued', 'mira', { status: 'queued', held: 'pause' });
  state.turns.cut = { ...turn('cut', 'lane', { status: 'stopped' }), position: 6 };
  state.messages.push(
    said('bad', 'user', 'Try this'),
    said('queued', 'user', 'And this'),
    said('cut', 'user', '@Lane'),
  );
  const { items } = view(alone(state));
  expect(ids(items).slice(-5)).toEqual(['bad-user', 'a-bad', 'queued-user', 'cut-user', 'a-cut']);
  expect(custom(byId(items, 'a-bad')).ending).toMatchObject({ word: 'Failed', act: 'retry' });
  expect(text(byId(items, 'a-cut'))).toBe('');
});

test('a stopped turn offers Continue until its carry-on runs, also while that waits or was stopped', () => {
  const state = round();
  state.turns.cut = { ...turn('cut', 'lane', { status: 'stopped' }), position: 5 };
  state.turns.next = turn('next', 'lane', { status: 'queued', held: 'pause', answers: 'cut' });
  state.messages.push(said('cut', 'user', '@Lane'), said('next', 'gathr', 'Stop now ended your turn.'));
  const ending = () => custom(byId(view(state).all, 'a-cut')).ending;
  expect(ending()).toMatchObject({ act: 'continue' });

  // Stopped before it ever started (its task cancelled while it waited): nothing carried on.
  state.turns.next = { ...state.turns.next, status: 'stopped', held: null, position: 6 };
  expect(ending()).toMatchObject({ act: 'continue' });

  // The carry-on is the next message: the stopped one keeps nothing.
  state.turns.next = { ...state.turns.next, status: 'running' };
  expect(ending()).toBeUndefined();
});

test('a message that failed to send is drawn again when its failure changes, and kept when it does not', () => {
  const state = round();
  const cache = new Map();
  const unsent = (error: string) => [{ id: '1', text: 'Try', failure: new Failure(error, null, null) }];
  const first = toThread(state, agents, unsent('Send failed'), cache).items.at(-1);
  expect(toThread(state, agents, unsent('Send failed'), cache).items.at(-1)).toBe(first);
  const next = toThread(state, agents, unsent('Connection failed'), cache).items.at(-1);
  expect(custom(next).unsent?.failure.error).toBe('Connection failed');
});

test('the lead is headed by its role, whatever task its turn runs on; a member by its task', () => {
  const state = round();
  state.turns.mira7 = { ...turn('mira7', 'mira', { task: 7, answers: 'rowan' }), position: 5 };
  state.tasks.push(task(7, 'mira', 'rowan', { giver: 'rowan', what: 'Which notice?' }));
  state.messages.push(said('mira7', 'assistant', 'Asking the person'));
  const { items } = view(state);
  expect(custom(byId(items, 'a-mira7'))).toMatchObject({ role: 'supervisor', head: true });
  expect(custom(byId(items, 'a-mira7')).on).toBeUndefined();
  // A member is headed by its task where the thread is another's; in its task's own thread, by nothing.
  expect(custom(byId(items, 'a-kit')).on).toBeUndefined();
  expect(
    custom(
      byId(
        view({
          ...state,
          turns: { ...state.turns, kit: { ...state.turns.kit, topic: state.turns.ask.topic } },
        }).all,
        'a-kit',
      ),
    ).on,
  ).toBe('Task 2');
});

const thought = (turnId: string, id: string, body: ThinkingBody, time = at()): Message => ({
  id,
  turn: turnId,
  time,
  role: 'assistant',
  kind: 'thinking',
  body,
});

test('a thought is a step that says how long it took, never counted with what the turn ran; under a second it is left out (0042)', () => {
  const state = round();
  const t0 = Date.UTC(2026, 10, 8, 10, 0, 0);
  const iso = (s: number) => new Date(t0 + s * 1000).toISOString();
  state.turns.lead = { ...turn('lead', 'mira', { created: iso(0) }), position: 9 };
  state.messages.push(
    thought('lead', 'th1', { status: 'done', duration_ms: 9_400, tokens: 204 }, iso(0)),
    tool('lead', 't1', 'Read', {}, iso(10)),
    thought('lead', 'th2', { status: 'done', duration_ms: 300 }, iso(11)),
    tool('lead', 't2', 'Bash', {}, iso(12)),
    thought('lead', 'th3', { status: 'stopped', duration_ms: 72_000 }, iso(13)),
    // Left running by a service that died, then stopped at the next start: how long isn't known.
    thought('lead', 'th4', { status: 'stopped' }, iso(14)),
  );
  state.turns.lead.status = 'stopped';
  const block = byId(view(state).all, 'a-lead');
  // A thought is never the turn's message, and one that ends the turn still counts in its time.
  expect(text(block)).toBe('');
  const work = custom(block).work;
  expect(work).toMatchObject({ label: 'Worked for 1m 25s', notes: ['2 steps'] });
  expect(work?.steps).toEqual([
    { id: 'th1', label: 'Thought for 9s', state: 'ok' },
    { id: 't1', label: 'Read a file', doing: 'Reading a file', state: 'ok' },
    { id: 't2', label: 'Ran a command', doing: 'Running a command', state: 'ok' },
    { id: 'th3', label: 'Thought for 1m 12s', state: 'stopped' },
  ]);

  // A turn that only thought, for under a second, has nothing to show: no row.
  const only = round();
  only.turns.lead = { ...turn('lead', 'mira', { status: 'failed' }), position: 9 };
  only.messages.push(thought('lead', 'th1', { status: 'done', duration_ms: 20 }));
  expect(custom(byId(view(only).all, 'a-lead')).work).toBeUndefined();
});

test('while a thought runs the turn’s row says Thinking; its end over the socket makes it the step', () => {
  let state = round();
  state.turns.kit.status = 'running';
  state.messages.push(tool('kit', 'k1', 'Edit'), thought('kit', 'th', { status: 'running' }));
  const running = custom(byId(view(state).all, 'a-kit')).work;
  expect(running).toMatchObject({ running: true, label: 'Thinking', notes: ['1 step'] });
  expect(running?.steps.at(-1)).toEqual({ id: 'th', label: 'Thinking', state: 'running' });

  state = apply(state, {
    type: 'tool',
    chat: 'c',
    message: 'th',
    body: { status: 'done', duration_ms: 6_000 },
  });
  const after = custom(byId(view(state).all, 'a-kit')).work;
  expect(after).toMatchObject({ running: true, label: 'Working', notes: ['1 step'] });
  expect(after?.steps.at(-1)).toMatchObject({ label: 'Thought for 6s', state: 'ok' });
});

test('thoughts with nothing between them are one step, with their time together (0042)', () => {
  const state = round();
  state.turns.kit.status = 'running';
  state.messages.push(
    thought('kit', 'a', { status: 'done', duration_ms: 21_700 }),
    thought('kit', 'b', { status: 'done', duration_ms: 400 }),
    thought('kit', 'c', { status: 'done', duration_ms: 40_300 }),
    tool('kit', 'k1', 'Edit'),
    thought('kit', 'd', { status: 'done', duration_ms: 5_000 }),
    thought('kit', 'e', { status: 'running' }),
  );
  const work = custom(byId(view(state).all, 'a-kit')).work;
  expect(work).toMatchObject({ running: true, label: 'Thinking' });
  expect(work?.steps.slice(-3)).toMatchObject([
    { id: 'a', label: 'Thought for 1m 02s', state: 'ok' },
    { id: 'k1' },
    { id: 'd', label: 'Thinking', state: 'running' },
  ]);

  // The running one's end makes the pair one finished thought.
  const after = apply(state, {
    type: 'tool',
    chat: 'c',
    message: 'e',
    body: { status: 'stopped', duration_ms: 2_000 },
  });
  expect(custom(byId(view(after).all, 'a-kit')).work?.steps.at(-1)).toMatchObject({
    id: 'd',
    label: 'Thought for 7s',
    state: 'stopped',
  });
});

test('a step opens to what it ran, whole, and the first lines of what came back (0044)', () => {
  const state = round();
  state.turns.lead = { ...turn('lead', 'mira'), position: 5 };
  state.messages.push(
    tool('lead', 't1', 'Bash', {
      input: { text: 'cargo nextest run', request: 'cargo nextest run \\n  --workspace --locked' },
      result: { text: 'Summary 360 tests run: 360 passed', lines: 41, more: true },
    }),
    tool('lead', 't2', 'Read', {
      input: { text: 'AGENTS.md' },
      result: { text: '212 lines', lines: 1, more: false },
    }),
    tool('lead', 't3', 'Bash', { status: 'error', input: { text: 'npx vitest run' } }),
    { ...said('lead', 'assistant', 'Done.') },
  );
  const steps = custom(byId(view(state).all, 'a-lead')).work?.steps;
  expect(steps).toEqual([
    {
      id: 't1',
      label: 'Ran',
      doing: 'Running',
      detail: 'cargo nextest run',
      state: 'ok',
      request: 'cargo nextest run \\n  --workspace --locked',
      result: 'Summary 360 tests run: 360 passed\n…',
    },
    {
      id: 't2',
      label: 'Read',
      doing: 'Reading',
      detail: 'AGENTS.md',
      state: 'ok',
      request: 'AGENTS.md',
      result: '212 lines',
    },
    // A step that kept no result says nothing came back rather than inventing one.
    {
      id: 't3',
      label: 'Ran',
      doing: 'Running',
      detail: 'npx vitest run',
      state: 'error',
      request: 'npx vitest run',
    },
  ]);
});
