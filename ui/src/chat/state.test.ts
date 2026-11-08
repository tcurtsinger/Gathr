import { expect, test } from 'vitest';
import { apply, empty } from './state';

const update = (status: 'running' | 'failed', agent: string | null) =>
  ({ type: 'turn', chat: 'c', turn: 't', status, agent }) as const;

test('a turn sent from another window knows its agent from its live updates alone', () => {
  const failed = apply(apply(empty, update('running', 'a')), update('failed', 'a'));
  expect(failed.turns.t).toMatchObject({ status: 'failed', agent: 'a' });

  const noAgent = apply(empty, update('failed', null));
  expect(noAgent.turns.t).toMatchObject({ status: 'failed', agent: null });
});
