import { useAuiState } from '@assistant-ui/react';
import { StreamdownTextPrimitive } from '@assistant-ui/react-streamdown';
import { code } from '@streamdown/code';
import { type ComponentProps, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { defaultRehypePlugins, defaultRemarkPlugins, Streamdown } from 'streamdown';
import { blockSplitter } from './blocks';
import './markdown.css';

interface Node {
  type: string;
  children?: Node[];
}

/** An agent's raw HTML is shown as the text it is, never parsed or run (0030). */
function htmlAsText() {
  const walk = (node: Node) => {
    if (node.type === 'html') node.type = 'text';
    node.children?.forEach(walk);
  };
  return walk;
}

const remarkPlugins = [...Object.values(defaultRemarkPlugins), htmlAsText];
// With no raw HTML left, rehype-raw and the sanitizer have nothing to do, and rehype-raw re-parses
// every reply through parse5. Harden still drops unsafe link and image URLs (javascript: and the like).
const rehypePlugins = [defaultRehypePlugins.harden];

// A link leaves the window for the person's browser; an in-page one (a footnote) stays in the reply.
const components = {
  a: ({ node: _, ...props }: ComponentProps<'a'> & { node?: unknown }) =>
    // Harden adds target="_blank" to every link, so an in-page one drops it.
    props.href?.startsWith('#') ? (
      <a {...props} target={undefined} rel={undefined} />
    ) : (
      <a {...props} target="_blank" rel="noreferrer" />
    ),
};

// Code blocks keep Streamdown's language label, Copy and Download, as in the prototype; tables have none.
// Shiki colours a block once its language loads; until then it shows plain.
const controls = { table: false };
const plugins = { code };

/** Streamdown draws a code block's body through React.lazy, and React holds a revealed Suspense fallback
 * for 300 ms, so the first chat with code drew 300 ms late. One block drawn off the page at start loads
 * it, and Shiki with its themes, before any chat; each language's grammar still loads at first use. */
export function preloadCodeBlock() {
  createRoot(document.createElement('div')).render(
    <Streamdown plugins={plugins}>{'```text\n.\n```'}</Streamdown>,
  );
}

/** An agent's text, formatted by Streamdown as it streams (0030). */
export function MarkdownText() {
  // A reply already finished when it's drawn parses in one pass (a chat with hundreds of them opens
  // faster). One drawn while streaming stays in streaming mode, so its end never re-parses it whole.
  const running = useAuiState((s) => s.message.status?.type === 'running');
  const [mode] = useState<'static' | 'streaming'>(running ? 'streaming' : 'static');
  const [split] = useState(() => (running ? blockSplitter() : undefined));
  return (
    <StreamdownTextPrimitive
      mode={mode}
      parseMarkdownIntoBlocksFn={split}
      remarkPlugins={remarkPlugins}
      rehypePlugins={rehypePlugins}
      components={components}
      linkSafety={{ enabled: false }}
      controls={controls}
      // Streamdown caps a table at 300px and scrolls it, which hid its header row; it shows whole.
      tableMaxHeight={Number.POSITIVE_INFINITY}
      plugins={plugins}
      containerClassName="markdown"
    />
  );
}
