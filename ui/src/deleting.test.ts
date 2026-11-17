import { expect, test } from 'vitest';
import { type Deletable, type DeleteCounts, deleteBody, leftFailure } from './deleting';

const counts = (chats: number, tasks: number, projects: number, leads: string[] = []): DeleteCounts => ({
  chats,
  tasks,
  projects,
  leads,
});

// The owner's sentences (the remove mock-up), then the combinations it doesn't draw, in the same shape.
const cases: [Deletable, DeleteCounts, string][] = [
  ['project', counts(4, 0, 1), '4 chats will be deleted. Files are kept.'],
  ['project', counts(1, 0, 1), '1 chat will be deleted. Files are kept.'],
  ['project', counts(0, 0, 1), 'Files are kept.'],
  ['agent', counts(0, 2, 0), '2 tasks will be cancelled.'],
  ['agent', counts(0, 1, 0, ['Ann']), '1 task will be cancelled. Ann becomes lead.'],
  ['agent', counts(2, 0, 1), '1 project and 2 chats will be deleted. Files are kept.'],
  ['agent', counts(0, 0, 0), ''],
  ['agent', counts(0, 0, 0, ['Ann']), 'Ann becomes lead.'],
  [
    'agent',
    counts(0, 3, 0, ['Ann', 'Bo', 'Kai']),
    '3 tasks will be cancelled. Ann, Bo and Kai become leads.',
  ],
  ['agent', counts(1, 0, 0), '1 chat will be deleted. Files are kept.'],
  ['agent', counts(0, 0, 2), '2 projects will be deleted. Files are kept.'],
  [
    'agent',
    counts(3, 1, 2, ['Ann', 'Bo']),
    '2 projects and 3 chats will be deleted. 1 task will be cancelled. Ann and Bo become leads. Files are kept.',
  ],
];

test.each(cases)('a %s delete of %o says “%s”', (kind, c, words) => {
  expect(deleteBody(kind, c)).toBe(words);
});

test('what a delete left is one failure: its why without the reference the notice opens', () => {
  expect(leftFailure([])).toBeNull();
  const left = leftFailure([
    'made-up folder wasn’t deleted: in use (ref R-1111)',
    'other made-up folder wasn’t deleted: in use (ref R-2222)',
  ]);
  expect([left?.error, left?.why, left?.ref]).toEqual([
    'Some files weren’t deleted',
    'made-up folder wasn’t deleted: in use; other made-up folder wasn’t deleted: in use (ref R-2222)',
    'R-1111',
  ]);
});
