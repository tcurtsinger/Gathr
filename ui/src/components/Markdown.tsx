import { useAuiState } from '@assistant-ui/react';
import { StreamdownTextPrimitive } from '@assistant-ui/react-streamdown';
import { type ComponentProps, useState } from 'react';
import { defaultRehypePlugins, defaultRemarkPlugins } from 'streamdown';
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
// Code is plain pre and code: Streamdown's own code block loads lazily, and React holds that for 300 ms.
const components = {
  pre: 'pre' as const,
  code: 'code' as const,
  a: ({ node: _, ...props }: ComponentProps<'a'> & { node?: unknown }) =>
    // Harden adds target="_blank" to every link, so an in-page one drops it.
    props.href?.startsWith('#') ? (
      <a {...props} target={undefined} rel={undefined} />
    ) : (
      <a {...props} target="_blank" rel="noreferrer" />
    ),
};

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
      controls={false}
      containerClassName="markdown"
    />
  );
}
