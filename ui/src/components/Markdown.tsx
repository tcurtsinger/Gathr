import { Fragment, memo, useMemo } from 'react';
import ReactMarkdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import remarkParse from 'remark-parse';
import { unified } from 'unified';
import './markdown.css';

const plugins = [remarkGfm];
const parser = unified().use(remarkParse).use(remarkGfm);

// Links leave the window for the person's browser (the desktop window sends new tabs there).
const components: Components = {
  a: ({ node: _, ...props }) => <a {...props} target="_blank" rel="noreferrer" />,
};

/**
 * An agent's text, formatted (0027). Raw HTML stays text; unsafe link schemes are dropped.
 * While it streams, only the top-level blocks that changed render again.
 */
export function Markdown({ text, streaming = false }: { text: string; streaming?: boolean }) {
  const parts = useMemo(() => (streaming ? blocks(text) : null), [text, streaming]);
  return (
    <div className="markdown">
      {parts === null ? (
        <Block text={text} />
      ) : (
        // Blocks are in text order and only the last ones change, so their place is their key.
        // The newline between them is the one the whole text renders.
        parts.map((part, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: see above
          <Fragment key={i}>
            {i > 0 && '\n'}
            <Block text={part} />
          </Fragment>
        ))
      )}
    </div>
  );
}

const Block = memo(function Block({ text }: { text: string }) {
  return (
    <ReactMarkdown remarkPlugins={plugins} components={components}>
      {text}
    </ReactMarkdown>
  );
});

/**
 * The text's top-level blocks, as the whole text parses, each with the document's link
 * definitions so its references resolve alone. Null when footnotes need the whole text.
 */
function blocks(text: string): string[] | null {
  const nodes = parser.parse(text).children;
  if (nodes.some((n) => n.type === 'footnoteDefinition')) return null;
  const source = (n: (typeof nodes)[number]) => {
    const { start, end } = n.position ?? { start: { offset: 0, column: 1 }, end: { offset: 0 } };
    // From the start of its line, so an indented block keeps its indent.
    return text.slice((start.offset ?? 0) - (start.column - 1), end.offset);
  };
  const definitions = nodes.filter((n) => n.type === 'definition').map(source);
  return nodes.filter((n) => n.type !== 'definition').map((n) => [source(n), ...definitions].join('\n\n'));
}
