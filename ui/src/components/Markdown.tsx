import { code } from '@streamdown/code';
import { type ComponentProps, useMemo } from 'react';
import { createRoot } from 'react-dom/client';
import { defaultRehypePlugins, defaultRemarkPlugins, Streamdown } from 'streamdown';
import './markdown.css';

interface Node {
  type: string;
  value?: string;
  children?: Node[];
  data?: { hName: string; hProperties: { className: string } };
}

/** An agent's raw HTML is shown as the text it is, never parsed or run (0030). */
function htmlAsText() {
  const walk = (node: Node) => {
    if (node.type === 'html') node.type = 'text';
    node.children?.forEach(walk);
  };
  return walk;
}

const literal = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/**
 * Each @Name of the chat's agents in the words' text, as the blue mention a message to an agent opens with (0072), the
 * longest name first; never inside code or a link. A plugin from outside the registry's Markdown (0030).
 */
function mentions(names: readonly string[]) {
  const sorted = [...names].sort((a, b) => b.length - a.length).map(literal);
  const named = new RegExp(`(?<![\\w@])@(?:${sorted.join('|')})(?![\\w-])`, 'gi');
  const split = (node: Node): Node[] => {
    const text = node.value ?? '';
    const out: Node[] = [];
    let from = 0;
    for (const hit of text.matchAll(named)) {
      if (hit.index > from) out.push({ type: 'text', value: text.slice(from, hit.index) });
      const data = { hName: 'span', hProperties: { className: 'mention' } };
      out.push({ type: 'text', value: hit[0], data });
      from = hit.index + hit[0].length;
    }
    if (from < text.length) out.push({ type: 'text', value: text.slice(from) });
    return out;
  };
  const walk = (node: Node) => {
    if (node.type === 'link' || !node.children) return;
    node.children = node.children.flatMap((child) => (child.type === 'text' ? split(child) : [child]));
    node.children.forEach(walk);
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

/**
 * Words outside a message, a plan's text or a task's field, formatted as an agent's words in the chat are (the owner's
 * call of 2026-11-19): the same plugins and the same rules (markdown.css), at the size of where they sit. With
 * `names`, an @Name of one of them reads as a mention.
 */
export function Markdown({ text, className, names }: { text: string; className?: string; names?: string[] }) {
  const key = names?.join('\n');
  // The names go in as the plugin's options: Streamdown keeps one parser per plugins' names and options, so a
  // plugin made per chat would draw every chat with the first chat's names.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the same names make the same plugins
  const remark = useMemo(
    () =>
      names?.length ? [...remarkPlugins, [mentions, names] as [typeof mentions, string[]]] : remarkPlugins,
    [key],
  );
  return (
    <Streamdown
      mode="static"
      className={className ? `markdown ${className}` : 'markdown'}
      remarkPlugins={remark}
      rehypePlugins={rehypePlugins}
      components={components}
      linkSafety={{ enabled: false }}
      controls={controls}
      tableMaxHeight={Number.POSITIVE_INFINITY}
      plugins={plugins}
    >
      {text}
    </Streamdown>
  );
}
