// The person's questions in the chat (0073, 0075): an agent's in the bubble that asked it, which is the question's main
// message, Gathr's as its own message, and each answer and Discuss the person's own message in the question's thread,
// whenever it was sent.
import { expect, test } from 'vitest';
import type { Message } from '../api';
import { byId, custom, events, ids, round, said, task, text, turn, view } from './madeUp';

/** When a message was said, in µs, `ms` later. */
const us = (m: Message, ms = 0) => (Date.parse(m.time) + ms) * 1000;
const choices = [{ label: 'B' }, { label: 'A' }];

test('an agent’s question sits in its bubble; the answer is the person’s message once, when it was sent', () => {
  const state = round();
  const reply = state.messages[1];
  const question = task(5, 'mira', 'ask', {
    kind: 'person',
    owner: null,
    what: 'Which notice?',
    state: 'ready',
    form: 'choice',
    choices,
    created_us: us(reply),
  });
  state.tasks.push(question);
  let { all: items } = view(state);
  expect(custom(byId(items, 'a-ask')).asked).toEqual([5]);
  // The block that asked it is the question's main message, out of the ask's thread.
  expect(custom(byId(items, 'a-ask'))).toMatchObject({ topic: 't:5' });
  expect(items.some((m) => m.id === 'said-5')).toBe(false);

  // Answered before the members' turns ran; Mira hears it after them, in a turn of her own.
  state.tasks[2] = {
    ...question,
    state: 'done',
    reason: 'B',
    heard_in: 'heard',
    answer: { pick: ['B'], at: us(reply, 500) },
  };
  // The turn that heard the answer is about the question, as the service names it.
  state.turns.heard = { ...turn('heard', 'mira', { answers: 'ask', topic: 't:5' }), position: 9 };
  state.messages.push(
    said('heard', 'gathr', 'The person answered:\n- #5 Which notice?: Picked: B\nContinue.'),
    said('heard', 'assistant', 'B it is'),
  );
  ({ all: items } = view(state));
  const answers = items.filter((m) => m.role === 'user' && text(m) === 'B');
  expect(answers.map((m) => m.id)).toEqual(['said-5']);
  expect(ids(items).indexOf('said-5')).toBe(ids(items).indexOf('a-ask') + 1);
  expect(custom(answers[0]).time).toBe(new Date(us(reply, 500) / 1000).toISOString());
  // Nothing else says it: no line at Mira's later turn, and her turn's message from Gathr isn't drawn. Her turn is in
  // the question's thread, after the answer.
  expect(events(items)).toEqual([]);
  expect(ids(items).indexOf('a-heard')).toBe(ids(items).indexOf('said-5') + 1);
});

test('an answer sits above what it started, though both were saved in the same millisecond', () => {
  const state = round();
  const reply = state.messages[1];
  const sent = us(reply, 500) + 400;
  state.tasks.push(
    task(5, 'mira', 'ask', {
      kind: 'person',
      owner: null,
      what: 'Which notice?',
      state: 'done',
      reason: 'B',
      heard_in: 'heard',
      form: 'choice',
      choices,
      created_us: us(reply),
      answer: { pick: ['B'], at: sent },
    }),
  );
  state.turns.heard = { ...turn('heard', 'mira', { answers: 'ask', topic: 't:5' }), position: 9 };
  const opened = said('heard', 'gathr', 'The person answered:\n- #5 Which notice?: Picked: B\nContinue.');
  state.messages.push(
    { ...opened, time: new Date(Math.floor(sent / 1000)).toISOString() },
    said('heard', 'assistant', 'B'),
  );
  const { all: items } = view(state);
  expect(ids(items).indexOf('said-5')).toBe(ids(items).indexOf('a-ask') + 1);
  expect(ids(items).indexOf('a-heard')).toBe(ids(items).indexOf('said-5') + 1);
});

test('Gathr’s question is Gathr’s own message where it was asked, and the person’s answer sits below it', () => {
  const state = round();
  const kit = state.messages.at(-1) as Message;
  const question = task(7, 'kit', 'kit', {
    kind: 'person',
    giver: null,
    owner: null,
    given_in: null,
    what: 'Let it go on?',
    why: 'Task #2 adds 1,240 lines.',
    state: 'ready',
    form: 'choice',
    choices,
    created_us: us(kit, 200),
  });
  state.tasks.push(question);
  const { items } = view(state);
  expect(ids(items).slice(-1)).toEqual(['q-7']);
  expect(custom(items.at(-1))).toMatchObject({ gathr: true, asked: [7] });

  state.tasks[2] = { ...question, state: 'done', reason: 'B', answer: { pick: ['B'], at: us(kit, 400) } };
  const { all } = view(state);
  expect(ids(all).slice(-2)).toEqual(['q-7', 'said-7']);
  expect(text(all.at(-1))).toBe('B');
  expect(custom(all.at(-1)).reply).toBe(true);
});

test('each Discuss and the answer after it are the person’s own messages, each when it was sent', () => {
  const state = round();
  const reply = state.messages[1];
  // Rowan's request #3, filed by Mira as #12; the person's Discuss woke her turn on #3.
  state.turns.talk = { ...turn('talk', 'mira', { task: 3, answers: 'ask', topic: 't:12' }), position: 6 };
  const talk = 'The person wants to discuss:\n- #12 Which notice text should ship?: Does it retry by itself?';
  state.messages.push(said('talk', 'gathr', talk), said('talk', 'assistant', 'It does'));
  const discussed = [{ text: 'Does it retry by itself?', turn: 'talk', at: us(reply, 300) }];
  const question = task(12, 'mira', 'ask', {
    kind: 'person',
    owner: null,
    what: 'Which notice text should ship?',
    created_us: us(reply),
  });
  state.tasks.push(task(3, 'mira', 'rowan', { kind: 'person_request', giver: 'rowan', state: 'held' }), {
    ...question,
    state: 'held',
    heard_in: 'talk',
    answer: { discussed },
  });
  let { all: items } = view(state);
  // In the question's thread: the Discuss, then the lead's turn that heard it.
  expect(ids(items).slice(ids(items).indexOf('a-ask'), ids(items).indexOf('a-ask') + 3)).toEqual([
    'a-ask',
    'said-12-0',
    'a-talk',
  ]);
  expect(text(byId(items, 'said-12-0'))).toBe('Does it retry by itself?');

  state.tasks[3] = {
    ...question,
    state: 'done',
    reason: 'A',
    answer: { pick: ['A'], text: 'and say we retry', at: us(reply, 600), discussed },
  };
  ({ all: items } = view(state));
  expect(ids(items).slice(ids(items).indexOf('a-ask'), ids(items).indexOf('a-ask') + 4)).toEqual([
    'a-ask',
    'said-12-0',
    'said-12',
    'a-talk',
  ]);
  expect(text(byId(items, 'said-12'))).toBe('A\nand say we retry');
  expect(events(items)).toEqual([]);
});

test('a request the person’s answer settled reads no “completed” line in its member’s hear-back', () => {
  const state = round();
  state.tasks.push(
    task(3, 'mira', 'rowan', { kind: 'person_request', giver: 'rowan', what: 'Which notice?' }),
    task(12, 'mira', 'ask', { kind: 'person', owner: null, request: 3, what: 'Which notice text?' }),
  );
  state.turns.back = { ...turn('back', 'rowan', { answers: 'rowan' }), position: 5 };
  const back =
    'The task you gave is settled:\n- #3 Mira, Which notice?: done: the person answered #12\nRead.';
  state.messages.push(said('back', 'gathr', back), said('back', 'assistant', 'Using it'));
  const { items } = view(state);
  expect(events(items)).toEqual([]);
  expect(ids(items).slice(-1)).toEqual(['a-back']);
});

test('what the lead is told after the person’s size change isn’t drawn: Gathr’s own line says it', () => {
  const state = round();
  state.turns.told = { ...turn('told', 'mira'), position: 5 };
  const told = 'The person made "Fix the reconnect" new, so it waits for a plan. Write the plan with plan.';
  state.messages.push(said('told', 'gathr', told), said('told', 'assistant', 'Writing it'));
  const { items } = view(state);
  expect(events(items)).toEqual([]);
  expect(ids(items).slice(-1)).toEqual(['a-told']);
});
test('a question a split turn asked shows once, in the block it was asked in', () => {
  const state = round();
  state.turns.kit.status = 'running';
  state.tasks.push(task(9, 'kit', 'kit', { owner: null, giver: 'kit', kind: 'person', what: 'Ship it?' }));
  state.messages.push(said('kit', 'user', '@Kit one thing'), said('kit', 'assistant', 'Kept'));
  const { all: items } = view(state);
  // The block that asked it is the question's main message; the person's message is one too, and what came after it
  // is a main message of its own (0078).
  expect(ids(items).slice(-3)).toEqual(['a-kit', 'kit-user', 'a-kit-1']);
  expect(custom(byId(items, 'a-kit-1')).reply).toBeUndefined();
  expect(items.map((m) => custom(m).asked).filter(Boolean)).toEqual([[9]]);
  expect(custom(byId(items, 'a-kit')).asked).toEqual([9]);
});
