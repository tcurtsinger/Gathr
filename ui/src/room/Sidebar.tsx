import { File as FileIcon, Files, Filter, House, Search, X } from 'lucide-react';
import { type ReactNode, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { api, type FileTree } from '../api';
import { ChangesMark } from '../components/ChangesMark';
import { DesignsMark } from '../components/DesignsMark';
import { pathSteps as steps, onWindows as windows } from '../components/files';
import { IconButton } from '../components/IconButton';
import { ResizeHandle } from '../components/ResizeHandle';
import { widest } from '../components/workspace';
import { Board } from './Board';
import { Changes, ChangesTile } from './Changes';
import { Designs, DesignTab } from './Design';
import { FileTab, type Opened, Tree } from './Files';
import { Find } from './Find';
import { isPage } from './PageFrame';
import type { OnAnswer } from './Question';
import type { RoomState } from './useRoom';
import './panel.css';

/** A chat's open tabs, and the one shown: Home, Files, Design, Changes, or a file's or design's path. */
interface Tabs {
  files: boolean;
  design: boolean;
  changes: boolean;
  open: Opened[];
  at: string;
}
const HOME = 'home';
const FILES = 'files';
const DESIGN = 'design';
const CHANGES = 'changes';
const FRESH: Tabs = { files: false, design: false, changes: false, open: [], at: HOME };
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

/** The one way a file or a design opens: in its own tab, or the tab it already has, at the option asked for. */
function show(chat: string, file: Opened) {
  const tabs = tabsOf(chat);
  const known = tabs.open.some((f) => f.path === file.path);
  const again = (f: Opened) => (f.path === file.path ? { ...file, page: file.page ?? f.page } : f);
  const open = known ? tabs.open.map(again) : [...tabs.open, file];
  change(chat, { ...tabs, open, at: file.path });
}

/** Opens a design of the chat's project in its own tab, as its Designs tab does, at the option page. */
export const openDesign = (chat: string, path: string, name: string, page?: string) =>
  show(chat, { path, crumbs: [name], design: true, page });

/**
 * Opens a file the chat names (a link, a tile, a file on a question) as the tree would: a full path, or one in
 * `agent`'s folder, its crumbs from the chat's top folder it is in. A page of a design opens the design at that
 * option. The service says whether it may be shown.
 */
export async function openTab(chat: string, path: string, agent?: string) {
  const tree = await api.tree(chat);
  const file = place(tree, path, agent);
  const designs = tree.designs && steps(tree.designs.path);
  const parts = steps(file.path);
  const same = (a: string, b: string) => (windows(file.path) ? a.toLowerCase() === b.toLowerCase() : a === b);
  const inDesign =
    designs && parts.length === designs.length + 2 && designs.every((p, i) => same(p, parts[i]));
  if (!tree.designs || !inDesign || !isPage(file.path)) return show(chat, file);
  const name = parts[designs.length];
  const folder = `${tree.designs.path}${windows(file.path) ? '\\' : '/'}${name}`;
  show(chat, { path: folder, crumbs: [name], design: true, page: file.path });
}

/** Opens the chat's Changes tab, as Home's tile does. */
export const openChanges = (chat: string) => change(chat, { ...tabsOf(chat), changes: true, at: CHANGES });

function place(tree: FileTree, path: string, agent?: string): Opened {
  const full = windows(path) || path.startsWith('/');
  const base = full ? undefined : tree.agents.find((a) => a.agent === agent)?.path;
  const at = base ? `${base}/${path}` : path;
  if (!full && !base) return { path, crumbs: steps(path) };
  const parts = steps(at);
  if (windows(at)) parts[0] = parts[0].toUpperCase();
  const whole = windows(at) ? parts.join('\\') : `/${parts.join('/')}`;
  const same = (a: string, b: string) => (windows(at) ? a.toLowerCase() === b.toLowerCase() : a === b);
  const tops = [tree.project, tree.shared, tree.designs, ...tree.agents].flatMap((t) => (t ? [t] : []));
  // The deepest top folder it is in names it.
  const top = tops
    .map((t) => ({ name: t.name, parts: steps(t.path) }))
    .filter((t) => t.parts.length < parts.length && t.parts.every((p, i) => same(p, parts[i])))
    .sort((a, b) => b.parts.length - a.parts.length)[0];
  const crumbs = top ? [top.name, ...parts.slice(top.parts.length)] : [parts.at(-1) ?? whole];
  return { path: whole, crumbs };
}

/** The panel beside the chat: Home first, always there, then Files, Design, Changes and each open file or design as tabs. */
export function RoomSidebar(props: PanelProps) {
  return (
    <aside className="workspace" aria-label="Chat sidebar">
      <ResizeHandle label="Resize chat sidebar" side="left" name="--workspace-width" min={360} max={widest} />
      <Panel key={props.chat} {...props} />
    </aside>
  );
}

interface PanelProps {
  chat: string;
  room: RoomState;
  onAnswer: OnAnswer;
  /** Sends a note on a design, as the chat's own send does. */
  onNote: (text: string, agent?: string) => void;
}

function Panel({ chat, room, onAnswer, onNote }: PanelProps) {
  const tabs = useSyncExternalStore(listen, () => tabsOf(chat));
  const order = [
    HOME,
    ...(tabs.files ? [FILES] : []),
    ...(tabs.design ? [DESIGN] : []),
    ...(tabs.changes ? [CHANGES] : []),
    ...tabs.open.map((f) => f.path),
  ];
  const go = (at: string) => change(chat, { ...tabs, at });
  const close = (key: string) => {
    const i = order.indexOf(key);
    const at = key === tabs.at ? (order[i + 1] ?? order[i - 1]) : tabs.at;
    change(chat, {
      files: tabs.files && key !== FILES,
      design: tabs.design && key !== DESIGN,
      changes: tabs.changes && key !== CHANGES,
      open: tabs.open.filter((f) => f.path !== key),
      at,
    });
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
          {tabs.design && tab(DESIGN, <DesignsMark />, 'Designs')}
          {tabs.changes && tab(CHANGES, <ChangesMark />, 'Changes')}
          <div className="workspace-files">
            {tabs.open.map((f) =>
              tab(
                f.path,
                f.design ? <DesignsMark /> : <FileIcon aria-hidden="true" />,
                f.crumbs.at(-1) ?? f.path,
              ),
            )}
          </div>
        </div>
      </div>
      {view(
        HOME,
        <div className="home">
          <Find
            chat={chat}
            icon={<Search aria-hidden="true" />}
            label="Find file"
            onOpen={(f) => show(chat, f)}
          >
            <h2>Open</h2>
            <div className="home-tiles">
              <button
                type="button"
                className="home-tile"
                onClick={() => change(chat, { ...tabs, files: true, at: FILES })}
              >
                <Files aria-hidden="true" />
                Files
              </button>
              <button
                type="button"
                className="home-tile"
                onClick={() => change(chat, { ...tabs, design: true, at: DESIGN })}
              >
                <DesignsMark />
                Designs
              </button>
              <ChangesTile
                chat={chat}
                room={room}
                open={tabs.at === HOME}
                onOpen={() => change(chat, { ...tabs, changes: true, at: CHANGES })}
              />
            </div>
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
      {tabs.design &&
        view(
          DESIGN,
          <Designs
            chat={chat}
            room={room}
            open={tabs.at === DESIGN}
            onOpen={(d) => show(chat, { path: d.path, crumbs: [d.name], design: true })}
          />,
        )}
      {tabs.changes && view(CHANGES, <Changes chat={chat} room={room} open={tabs.at === CHANGES} />)}
      {tabs.open.map((f) =>
        view(
          f.path,
          f.design ? (
            <DesignTab
              chat={chat}
              design={f}
              room={room}
              open={tabs.at === f.path}
              onShow={(page) => show(chat, { ...f, page })}
              onAnswer={onAnswer}
              onNote={onNote}
            />
          ) : (
            <FileTab chat={chat} file={f} />
          ),
        ),
      )}
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
