import { expect, test } from 'vitest';
import { apply, empty, sent } from './state';

const update = (status: 'running' | 'failed') => ({ type: 'turn', chat: 'c', turn: 't', status }) as const;

test('the agent a turn was sent to survives its live updates, whichever arrives first', () => {
  const answerFirst = apply(apply(sent(empty, 't', 'queued', 'a'), update('running')), update('failed'));
  expect(answerFirst.turns.t).toMatchObject({ status: 'failed', agent: 'a' });

  const liveFirst = sent(apply(empty, update('running')), 't', 'queued', 'a');
  expect(liveFirst.turns.t).toMatchObject({ status: 'running', agent: 'a' });
});
