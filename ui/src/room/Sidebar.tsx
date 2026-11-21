import { File as FileIcon, Files, Filter, House, Search, X } from 'lucide-react';
import { type ReactNode, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { api, type FileTree } from '../api';
import { IconButton } from '../components/IconButton';
import { ResizeHandle } from '../components/ResizeHandle';
import { widest } from '../components/workspace';
import { Board } from './Board';
import { FileTab, type Opened, Tree } from './Files';
import { Find } from './Find';
import './panel.css';

/** A chat's open tabs, and the one shown: Home, Files, or a file's path. */
interface Tabs {
  files: boolean;
  open: Opened[];
  at: string;
}
const HOME = 'home';
const FILES = 'files';
const FRESH: Tabs = { files: false, open: [], at: HOME };
// Each chat's tabs, while the window is open, and the panel that shows them.
const kept = new Map<string, Tabs>();
const heard = new Set<() => void>();
const tabsOf = (chat: string) => kept.get(chat) ?? FRESH;
const listen = (heed: () => void) => {
  heard.add(heed);
  return () => heard.delete(heed);
};
function change(chat: string, next: Tabs) {
  kept.set(chat, next);
  for (const heed of heard) heed();
}

/** The one way a file opens: in its own tab, or the tab it already has. */
function show(chat: string, file: Opened) {
  const tabs = tabsOf(chat);
  const known = tabs.open.some((f) => f.path === file.path);
  change(chat, { ...tabs, open: known ? tabs.open : [...tabs.open, file], at: file.path });
}

/**
 * Opens a file the chat names (a link, a tile, a file on a question) as the tree would: a full path, or one in
 * `agent`'s folder, its crumbs from the chat's top folder it is in. The service says whether it may be shown.
 */
export async function openTab(chat: string, path: string, agent?: string) {
  show(chat, place(await api.tree(chat), path, agent));
}

const windows = (path: string) => /^[A-Za-z]:/.test(path);
/** The path's parts, `.` and `..` followed. */
function steps(path: string) {
  const out: string[] = [];
  for (const s of path.split(/[\\/]/).filter(Boolean)) {
    if (s === '..') out.pop();
    else if (s !== '.') out.push(s);
  }
  return out;
}

function place(tree: FileTree, path: string, agent?: string): Opened {
  const full = windows(path) || path.startsWith('/');
  const base = full ? undefined : tree.agents.find((a) => a.agent === agent)?.path;
  const at = base ? `${base}/${path}` : path;
  if (!full && !base) return { path, crumbs: steps(path) };
  const parts = steps(at);
  if (windows(at)) parts[0] = parts[0].toUpperCase();
  const whole = windows(at) ? parts.join('\\') : `/${parts.join('/')}`;
  const same = (a: string, b: string) => (windows(at) ? a.toLowerCase() === b.toLowerCase() : a === b);
  const tops = [tree.project, tree.shared, ...tree.agents].flatMap((t) => (t ? [t] : []));
  // The deepest top folder it is in names it.
  const top = tops
    .map((t) => ({ name: t.name, parts: steps(t.path) }))
    .filter((t) => t.parts.length < parts.length && t.parts.every((p, i) => same(p, parts[i])))
    .sort((a, b) => b.parts.length - a.parts.length)[0];
  const crumbs = top ? [top.name, ...parts.slice(top.parts.length)] : [parts.at(-1) ?? whole];
  return { path: whole, crumbs };
}

/** The panel beside the chat: Home first, always there, then Files and each open file as tabs. */
export function RoomSidebar({ chat }: { chat: string }) {
  return (
    <aside className="workspace" aria-label="Chat sidebar">
      <ResizeHandle label="Resize chat sidebar" side="left" name="--workspace-width" min={360} max={widest} />
      <Panel key={chat} chat={chat} />
    </aside>
  );
}

function Panel({ chat }: { chat: string }) {
  const tabs = useSyncExternalStore(listen, () => tabsOf(chat));
  const order = [HOME, ...(tabs.files ? [FILES] : []), ...tabs.open.map((f) => f.path)];
  const go = (at: string) => change(chat, { ...tabs, at });
  const close = (key: string) => {
    const i = order.indexOf(key);
    const at = key === tabs.at ? (order[i + 1] ?? order[i - 1]) : tabs.at;
    change(chat, { files: tabs.files && key !== FILES, open: tabs.open.filter((f) => f.path !== key), at });
  };
  const id = (key: string) => order.indexOf(key);
  const tab = (key: string, icon: ReactNode, name: string, closable = true) => (
    <Tab
      key={key}
      id={id(key)}
      selected={tabs.at === key}
      icon={icon}
      name={name}
      onSelect={() => go(key)}
      onClose={closable ? () => close(key) : undefined}
    />
  );
  const view = (key: string, body: ReactNode) => (
    <section
      key={key}
      id={`room-view-${id(key)}`}
      role="tabpanel"
      aria-labelledby={`room-tab-${id(key)}`}
      className="workspace-view"
      hidden={tabs.at !== key}
    >
      {body}
    </section>
  );
  return (
    <>
      <div className="workspace-tabs">
        <div className="workspace-tablist" role="tablist" aria-label="Sidebar tabs">
          {tab(HOME, <House aria-hidden="true" />, 'Home', false)}
          {tabs.files && tab(FILES, <Files aria-hidden="true" />, 'Files')}
          <div className="workspace-files">
            {tabs.open.map((f) => tab(f.path, <FileIcon aria-hidden="true" />, f.crumbs.at(-1) ?? f.path))}
          </div>
        </div>
      </div>
      {view(
        HOME,
        <div className="home">
          <Find
            chat={chat}
            icon={<Search aria-hidden="true" />}
            label="Find a file"
            onOpen={(f) => show(chat, f)}
          >
            <h2>Open</h2>
            <button
              type="button"
              className="home-tile"
              onClick={() => change(chat, { ...tabs, files: true, at: FILES })}
            >
              <Files aria-hidden="true" />
              Files
            </button>
            <h2>Board</h2>
            <Board chat={chat} />
          </Find>
        </div>,
      )}
      {tabs.files &&
        view(
          FILES,
          <Find
            chat={chat}
            icon={<Filter aria-hidden="true" />}
            label="Filter files"
            onOpen={(f) => show(chat, f)}
          >
            <Tree chat={chat} onOpen={(file) => show(chat, file)} />
          </Find>,
        )}
      {tabs.open.map((f) => view(f.path, <FileTab chat={chat} file={f} />))}
    </>
  );
}

interface TabProps {
  id: number;
  selected: boolean;
  icon: ReactNode;
  name: string;
  onSelect: () => void;
  onClose?: () => void;
}

/**
 * One tab: the open one scrolls into sight. A tab scrolled partly out of sight isn't drawn, so the row never shows
 * a sliver of one, such as its bare close mark.
 */
function Tab({ id, selected, icon, name, onSelect, onClose }: TabProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [whole, setWhole] = useState(true);
  useEffect(() => {
    if (selected) ref.current?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [selected]);
  useEffect(() => {
    const tab = ref.current;
    if (!tab?.parentElement) return;
    const seen = new IntersectionObserver(([e]) => setWhole(e.intersectionRatio > 0.99), {
      root: tab.parentElement,
      threshold: [0.99, 1],
    });
    seen.observe(tab);
    return () => seen.disconnect();
  }, []);
  return (
    <div ref={ref} className="workspace-tab" data-selected={selected} data-whole={whole}>
      <button
        type="button"
        role="tab"
        id={`room-tab-${id}`}
        aria-selected={selected}
        aria-controls={`room-view-${id}`}
        onClick={onSelect}
      >
        {icon}
        <span>{name}</span>
      </button>
      {onClose && (
        <IconButton aria-label={`Close ${name}`} className="workspace-close" onClick={onClose}>
          <X />
        </IconButton>
      )}
    </div>
  );
}
