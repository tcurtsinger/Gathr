const fence = /^ {0,3}(`{3,}|~{3,})/;

/**
 * Markdown split at blank lines into blocks that parse alone, so a streaming reply parses only
 * its last block again (0027). A blank line inside a code fence, or before an indented line (a
 * list item's continuation), doesn't split.
 */
export function blocks(text: string): string[] {
  const lines = text.split('\n');
  const out: string[] = [];
  let block: string[] = [];
  let open: string | null = null;
  lines.forEach((line, i) => {
    const marker = line.match(fence)?.[1];
    if (marker && (open === null || (marker[0] === open[0] && marker.length >= open.length))) {
      open = open === null ? marker : null;
    }
    const next = lines.slice(i + 1).find((l) => l.trim() !== '');
    const ends = open === null && line.trim() === '' && !/^\s/.test(next ?? '');
    if (!ends) block.push(line);
    else if (block.length > 0) {
      out.push(block.join('\n'));
      block = [];
    }
  });
  if (block.length > 0) out.push(block.join('\n'));
  return out;
}
