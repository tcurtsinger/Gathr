// Temporary: proves a failing test names itself in the CI log (0022, S66). Removed next push.
import { expect, test } from 'vitest';

test('a deliberately failing test names itself in CI', () => {
  expect(1 + 1, 'the sum of one and one').toBe(3);
});
