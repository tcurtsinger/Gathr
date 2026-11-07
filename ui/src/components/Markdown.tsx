import { Fragment, memo, useMemo, useRef } from 'react';
import ReactMarkdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { blocks, type Parsed, parse } from './blocks';
import './markdown.css';

const plugins = [remarkGfm];

// Links leave the window for the person's browser (the desktop window sends new tabs there).
const components: Components = {
  a: ({ node: _, ...props }) => <a {...props} target="_blank" rel="noreferrer" />,
};

/**
 * An agent's text, formatted (0027). Raw HTML stays text; unsafe link schemes are dropped.
 * While it streams, only the top-level blocks that changed render again.
 */
export function Markdown({ text, streaming = false }: { text: string; streaming?: boolean }) {
  const last = useRef<Parsed | null>(null);
  const parts = useMemo(() => {
    if (!streaming) return null;
    last.current = parse(text, last.current);
    return blocks(last.current);
  }, [text, streaming]);
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
