import { code } from '@streamdown/code';
import { type ComponentProps, type MouseEvent, useContext, useMemo } from 'react';
import { createRoot } from 'react-dom/client';
import { defaultRehypePlugins, defaultRemarkPlugins, Streamdown } from 'streamdown';
import { linkedFile, OpenFile } from './files';
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

const FILE = '#file:';
/** A link to a file carries its path as an in-page address, which Harden lets through: it drops a file's own. */
function fileLinks() {
  const walk = (node: Node & { url?: string }) => {
    const path = node.type === 'link' && node.url ? linkedFile(node.url) : undefined;
    if (path) node.url = FILE + encodeURIComponent(path);
    node.children?.forEach(walk);
  };
  return walk;
}

const remarkPlugins = [...Object.values(defaultRemarkPlugins), htmlAsText, fileLinks];
// With no raw HTML left, rehype-raw and the sanitizer have nothing to do, and rehype-raw re-parses
// every reply through parse5. Harden still drops unsafe link and image URLs (javascript: and the like): a link it
// drops shows as its own words, with none of Harden's "[blocked]" after them and no hover line.
const [harden, hardening] = defaultRehypePlugins.harden as [unknown, object];
const rehypePlugins = [[harden, { ...hardening, linkBlockPolicy: 'text-only' }]] as ComponentProps<
  typeof Streamdown
>['rehypePlugins'];

// A link leaves the window for the person's browser; an in-page one (a footnote) stays in the reply; a file opens
// in the chat's panel, and outside a chat shows as its words.
function Link({ node: _, ...props }: ComponentProps<'a'> & { node?: unknown }) {
  const open = useContext(OpenFile);
  if (props.href?.startsWith(FILE)) {
    const path = decodeURIComponent(props.href.slice(FILE.length));
    if (!open) return <span>{props.children}</span>;
    const click = (e: MouseEvent) => {
      e.preventDefault();
      open(path);
    };
    return (
      <a href={props.href} onClick={click}>
        {props.children}
      </a>
    );
  }
  // Harden adds target="_blank" to every link, so an in-page one drops it.
  return props.href?.startsWith('#') ? (
    <a {...props} target={undefined} rel={undefined} />
  ) : (
    <a {...props} target="_blank" rel="noreferrer" />
  );
}
const components = { a: Link };

// A code block has one button, Copy, in its top right corner, and no bar, language label, Download or line numbers
// (the owner's pick, 2026-11-22; markdown.css hides the bar); tables have no buttons.
// Shiki colours a block once its language loads; until then it shows plain.
const controls = { table: false, code: { download: false } };
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
      lineNumbers={false}
      tableMaxHeight={Number.POSITIVE_INFINITY}
      plugins={plugins}
    >
      {text}
    </Streamdown>
  );
}
