import { parseMarkdownIntoBlocks } from 'streamdown';

// Streamdown's own splitter re-splits only the end of a text it split before, but it remembers one
// text for the whole page: with several replies streaming at once, every call misses and splits a
// whole reply again. Each streaming reply keeps its own memory here, by Streamdown's rule: blocks
// up to the last one ending in a blank line, two blocks back from the end, can't change as text is
// added. Footnotes and link definitions reach back across blocks, so those texts split whole.
const reachesBack = /\[\^[\w-]{1,200}\]|^ {0,3}\[[^\]]+\]:/m;

export function blockSplitter() {
  let input = '';
  let blocks: string[] = [];
  return (markdown: string): string[] => {
    const text = markdown.includes('\r') ? markdown.replace(/\r\n|\r/g, '\n') : markdown;
    if (text === input) return blocks;
    let keep = 0;
    if (text.startsWith(input) && !reachesBack.test(text)) {
      for (let i = blocks.length - 3; i >= 0 && !keep; i--) if (blocks[i].endsWith('\n\n')) keep = i + 1;
    }
    let at = 0;
    for (let i = 0; i < keep; i++) at += blocks[i].length;
    blocks = keep
      ? [...blocks.slice(0, keep), ...parseMarkdownIntoBlocks(text.slice(at))]
      : parseMarkdownIntoBlocks(text);
    input = text;
    return blocks;
  };
}
