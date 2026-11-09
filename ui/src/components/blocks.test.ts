import { parseMarkdownIntoBlocks } from 'streamdown';
import { expect, test } from 'vitest';
import { blockSplitter } from './blocks';

const reply = [
  '# Made-up report\n\nA paragraph with **bold** and `code`.\n\n',
  '- one\n- two\n  - nested\n\n1. first\n2. second\n\n',
  '```ts\nconst a = 1;\n\nconst b = 2;\n```\n\n',
  '| a | b |\n|---|---|\n| 1 | 2 |\n\n',
  '> quoted\n> still quoted\n\n',
  '<div>\n\nraw html stays text\n\n</div>\n\n',
  '$$\nx = 1\n\ny = 2\n$$\n\n',
  'Last paragraph, still streaming',
].join('');

test('a streaming reply splits into the same blocks at every length as splitting it whole', () => {
  const split = blockSplitter();
  for (let n = 1; n <= reply.length; n++) {
    const text = reply.slice(0, n);
    expect(split(text), `at ${n} characters`).toEqual(parseMarkdownIntoBlocks(text));
  }
});

test('two replies streaming in turns each keep their own blocks', () => {
  const [a, b] = [blockSplitter(), blockSplitter()];
  const other = reply.replaceAll('Made-up', 'Other');
  for (let n = 1; n <= reply.length; n += 7) {
    expect(a(reply.slice(0, n))).toEqual(parseMarkdownIntoBlocks(reply.slice(0, n)));
    expect(b(other.slice(0, n))).toEqual(parseMarkdownIntoBlocks(other.slice(0, n)));
  }
});
