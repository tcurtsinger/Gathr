import remarkGfm from 'remark-gfm';
import remarkParse from 'remark-parse';
import { unified } from 'unified';

const parser = unified().use(remarkParse).use(remarkGfm);

/** A text's top-level blocks, by type and where each starts (at its line's start) and ends. */
export interface Parsed {
  text: string;
  tops: { type: string; start: number; end: number }[];
}

/**
 * Parses `text` into its top-level blocks. When it only grew since `before`, only the last block
 * and the one before it can change: "1" after a list becomes "1." and joins it (0027). So
 * parsing starts again where the second-to-last block starts.
 */
export function parse(text: string, before: Parsed | null): Parsed {
  const grew = before !== null && before.tops.length > 2 && text.startsWith(before.text);
  const kept = grew ? before.tops.slice(0, -2) : [];
  const from = grew ? before.tops[before.tops.length - 2].start : 0;
  const tops = parser.parse(text.slice(from)).children.map((n) => {
    const start = n.position?.start ?? { offset: 0, column: 1 };
    return {
      type: n.type,
      start: from + (start.offset ?? 0) - (start.column - 1),
      end: from + (n.position?.end.offset ?? 0),
    };
  });
  return { text, tops: [...kept, ...tops] };
}

/**
 * Each block's source after the document's link definitions, so its references resolve alone.
 * Before, not after: a code fence still open at the end would take them in. Null when
 * footnotes need the whole text.
 */
export function blocks({ text, tops }: Parsed): string[] | null {
  if (tops.some((t) => t.type === 'footnoteDefinition')) return null;
  const source = (t: Parsed['tops'][number]) => text.slice(t.start, t.end);
  const definitions = tops.filter((t) => t.type === 'definition').map(source);
  return tops.filter((t) => t.type !== 'definition').map((t) => [...definitions, source(t)].join('\n\n'));
}
