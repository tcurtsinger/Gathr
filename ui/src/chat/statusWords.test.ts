import { expect, test } from 'vitest';
import { inWords } from './statusWords';

test('a pull request’s wait reads as the approved pages write it; any other wait as the service says it', () => {
  const said = [
    'CI on #18',
    '#18 to merge or close',
    '#18 to be up to date',
    'CI on 0a1b2c3',
    'Made-up Lead',
    'a check',
  ];
  expect(said.map(inWords)).toEqual([
    'PR #18 checks',
    'PR #18 to merge',
    'PR #18 to be up to date',
    'CI on 0a1b2c3',
    'Made-up Lead',
    'a check',
  ]);
});
