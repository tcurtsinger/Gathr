import { expect, test } from 'vitest';
import { blocks } from './blocks';

test('splits at blank lines, so finished paragraphs are parsed once', () => {
  expect(blocks('# Title\n\nOne.\nStill one.\n\n\nTwo.')).toEqual(['# Title', 'One.\nStill one.', 'Two.']);
});

test('keeps a fenced code block whole, blank lines and all', () => {
  const code = '```ts\nconst a = 1;\n\nconst b = 2;\n```';
  expect(blocks(`Before.\n\n${code}\n\nAfter.`)).toEqual(['Before.', code, 'After.']);
  expect(blocks('~~~\nopen\n\nstill open')).toEqual(['~~~\nopen\n\nstill open']);
});

test('keeps an indented continuation with the item it belongs to', () => {
  const item = '1. First\n\n    more of the first\n\n   - nested';
  expect(blocks(`${item}\n\n2. Second`)).toEqual([item, '2. Second']);
});
