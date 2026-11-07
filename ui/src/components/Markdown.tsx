import { memo } from 'react';
import ReactMarkdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { blocks } from './blocks';
import './markdown.css';

const plugins = [remarkGfm];

// Links leave the window for the person's browser (the desktop window sends new tabs there).
const components: Components = {
  a: ({ node: _, ...props }) => <a {...props} target="_blank" rel="noreferrer" />,
};

/** An agent's text, formatted (0027). Raw HTML stays text; unsafe link schemes are dropped. */
export function Markdown({ text }: { text: string }) {
  return (
    <div className="markdown">
      {blocks(text).map((block, i) => (
        // Blocks only grow at the end, so a finished block keeps its place and its parse.
        // biome-ignore lint/suspicious/noArrayIndexKey: see above
        <Block key={i} text={block} />
      ))}
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
