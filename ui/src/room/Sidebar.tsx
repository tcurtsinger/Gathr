import {
  ChevronDown,
  File as FileIcon,
  Folder,
  Globe,
  House,
  LoaderCircle,
  Search,
  SquareTerminal,
  X,
} from 'lucide-react';
import { type ReactNode, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { api, type FileTree } from '../api';
import { ChangesMark } from '../components/ChangesMark';
import { DesignsMark } from '../components/DesignsMark';
import { Face } from '../components/Face';
import { pathSteps as steps, onWindows as windows } from '../components/files';
import { IconButton } from '../components/IconButton';
import { ResizeHandle } from '../components/ResizeHandle';
import { widest } from '../components/workspace';
import { Board } from './Board';
import { BrowserTile, BrowserView } from './Browser';
import { Changes, ChangesTile, count, useChanges } from './Changes';
import { Designs, DesignTab, useDesigns } from './Design';
import { FileTab, type Opened, Tree } from './Files';
import { Find } from './Find';
import { isPage } from './PageFrame';
import { pageName, say, site, useBrowser } from './pages';
import type { OnAnswer } from './Question';
import { type StartFolder, TerminalTile, TerminalView } from './Terminal';
import { asked, openTerminal, tell, useTerminals } from './terminals';
import type { RoomState } from './useRoom';
import './panel.css';

/**
 * A chat's open tabs, and the one shown: Home, Files, Design, Changes, Browser, or a file's or design's path. The
 * browser's pages are the person's one browser's, the same in every chat: Browser shows the page it shows.
 */
interface Tabs {
  files: boolean;
  design: boolean;
  changes: boolean;
  browser: boolean;
  open: Opened[];
  at: string;
}
const HOME = 'home';
const FILES = 'files';
const DESIGN = 'design';
const CHANGES = 'changes';
const BROWSER = 'browser';
const FRESH: Tabs = { files: false, design: false, changes: false, browser: false, open: [], at: HOME };
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

/** The panel beside the chat: Home always there as an icon, then each view, page, file or design opened from it. */
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

/** A named tab's widest, the gap between tabs, and the "+N" list's button. */
const WIDEST = 180;
const GAP = 3;
const MORE = 44;
/** A tab's width beside its name: padding, icon and gap; the shown tab's close mark adds `CLOSE`. */
const AROUND = 41;
const CLOSE = 15;
/** The least room a tab is cut short to, rather than go in the list. */
const LEAST = 90;

/** A page's or a file's tab in the strip after the fixed ones, and its row in the "+N" list. */
interface Item {
  key: string;
  icon: ReactNode;
  name: string;
  where: string;
  selected: boolean;
  select: () => void;
  close: () => void;
  /** A browser page's id, for a page's tab. */
  page?: string;
}

let ruler: CanvasRenderingContext2D | null | undefined;
/** How wide `name` is in a tab, by the strip's own font. */
function nameWidth(name: string, strip: HTMLElement | null) {
  ruler ??= document.createElement('canvas').getContext('2d');
  if (!ruler || !strip) return name.length * 7;
  ruler.font = `13px ${getComputedStyle(strip).fontFamily}`;
  return ruler.measureText(name).width;
}

/** The tabs that fit in `width` as wide as their names, in order, the shown one always among them. */
function fitting(items: Item[], width: number, strip: HTMLElement | null) {
  const wide = items.map((i) =>
    Math.min(WIDEST, Math.ceil(nameWidth(i.name, strip)) + AROUND + (i.selected ? CLOSE : 0)),
  );
  const all = wide.reduce((sum, w) => sum + w + GAP, -GAP);
  if (all <= width) return items;
  const at = items.findIndex((i) => i.selected);
  let room = width - MORE - (at < 0 ? 0 : wide[at] + GAP);
  return items.filter((_, n) => {
    if (n === at) return true;
    if (wide[n] + GAP > room) {
      // The first that doesn't fit takes what room is left, cut short, when that shows enough of its name.
      const fits = room >= LEAST;
      room = 0;
      return fits;
    }
    room -= wide[n] + GAP;
    return true;
  });
}

function Panel({ chat, room, onAnswer, onNote }: PanelProps) {
  const tabs = useSyncExternalStore(listen, () => tabsOf(chat));
  const changes = useChanges(chat, room, true);
  const [tree, setTree] = useState<FileTree>();
  useEffect(() => {
    api.tree(chat).then(setTree, () => {});
  }, [chat]);
  const strip = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(Number.POSITIVE_INFINITY);
  useEffect(() => {
    const el = strip.current;
    if (!el) return;
    const seen = new ResizeObserver(([e]) => setWidth(e.contentRect.width));
    seen.observe(el);
    return () => seen.disconnect();
  }, []);
  const browser = useBrowser();
  const terminals = useTerminals(chat).list;
  const pageKey = (id: string) => `page:${id}`;
  const termKey = (id: string) => `term:${id}`;
  const order = [HOME, FILES, DESIGN, CHANGES, BROWSER, ...browser.pages.map((p) => pageKey(p.id))];
  order.push(...terminals.map((t) => termKey(t.id)));
  order.push(...tabs.open.map((f) => f.path));
  // Files, Designs, Changes and Browser are drawn once opened, and kept until their tab closes.
  const go = (at: string) =>
    change(chat, {
      ...tabs,
      files: tabs.files || at === FILES,
      design: tabs.design || at === DESIGN,
      changes: tabs.changes || at === CHANGES,
      browser: tabs.browser || at === BROWSER,
      at,
    });
  // A view's or a file's tab closes; when it was shown, the tab after it shows, or the one before, or Home.
  const close = (keys: string[]) => {
    const i = items.findIndex((t) => t.selected);
    const left = items.filter((t) => !keys.includes(t.key));
    const next =
      i >= 0 && keys.includes(items[i].key) ? (left[Math.min(i, left.length - 1)] ?? null) : undefined;
    if (next?.page) say({ type: 'show', page: next.page });
    change(chat, {
      files: tabs.files && !keys.includes(FILES),
      design: tabs.design && !keys.includes(DESIGN),
      changes: tabs.changes && !keys.includes(CHANGES),
      browser: tabs.browser && !keys.includes(BROWSER),
      open: tabs.open.filter((f) => !keys.includes(f.path)),
      at: next === undefined ? tabs.at : next === null ? HOME : next.page ? BROWSER : next.key,
    });
  };
  // The person's New shows the terminal it opens; one an agent starts waits for a look.
  const known = useRef<string[] | null>(null);
  useEffect(() => {
    const before = known.current;
    known.current = terminals.map((t) => t.id);
    if (!before) return;
    const fresh = terminals.find((t) => !t.agent && !before.includes(t.id));
    if (fresh && asked.delete(chat)) go(termKey(fresh.id));
    else if (tabs.at.startsWith('term:') && !terminals.some((t) => termKey(t.id) === tabs.at)) go(HOME);
  });
  const id = (key: string) => order.indexOf(key);
  const tab = (key: string, icon: ReactNode, name: string) => (
    <Tab key={key} id={id(key)} selected={tabs.at === key} icon={icon} name={name} onSelect={() => go(key)} />
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
  const nameOf = (f: Opened) => f.crumbs.at(-1) ?? f.path;
  const pages: Item[] = browser.pages.map((p) => ({
    key: pageKey(p.id),
    icon: p.loading ? (
      <LoaderCircle aria-hidden="true" className="workspace-loading" />
    ) : (
      <Globe aria-hidden="true" />
    ),
    name: pageName(p),
    where: site(p.url),
    selected: tabs.at === BROWSER && browser.shown === p.id,
    page: p.id,
    select: () => {
      say({ type: 'show', page: p.id });
      go(BROWSER);
    },
    close: () => say({ type: 'close', page: p.id }),
  }));
  const nameOfAgent = (agent: string) => room.members.find((m) => m.agent === agent)?.name ?? '';
  const terms: Item[] = terminals.map((t) => ({
    key: termKey(t.id),
    icon: t.agent ? (
      <Face agent={{ id: t.agent, name: nameOfAgent(t.agent) }} size={16} />
    ) : (
      <SquareTerminal aria-hidden="true" />
    ),
    name: t.name,
    where: t.folder,
    selected: tabs.at === termKey(t.id),
    select: () => go(termKey(t.id)),
    close: () => {
      close([termKey(t.id)]);
      tell(chat, { type: 'close', id: t.id });
    },
  }));
  const starts: StartFolder[] = [
    tree?.project ? { name: tree.project.name } : { name: 'Home' },
    ...(tree?.agents ?? []).map((a) => ({ name: a.name, path: a.path })),
  ];
  const files: Item[] = tabs.open.map((f) => ({
    key: f.path,
    icon: f.design ? <DesignsMark /> : <FileIcon aria-hidden="true" />,
    name: nameOf(f),
    where: f.crumbs.slice(0, -1).join(' / '),
    selected: tabs.at === f.path,
    select: () => go(f.path),
    close: () => close([f.path]),
  }));
  // Home's tiles open Files, Designs, Changes and Browser as tabs of their own, before the pages and files. Browser's
  // tab stands for the view only while it has no page, since each page has its own tab.
  const viewTab = (key: string, on: boolean, icon: ReactNode, name: string): Item[] =>
    on
      ? [
          {
            key,
            icon,
            name,
            where: '',
            selected: tabs.at === key,
            select: () => go(key),
            close: () => close([key]),
          },
        ]
      : [];
  const views: Item[] = [
    ...viewTab(FILES, tabs.files, <Folder aria-hidden="true" />, 'Files'),
    ...viewTab(DESIGN, tabs.design, <DesignsMark />, 'Designs'),
    ...viewTab(CHANGES, tabs.changes, <ChangesMark />, 'Changes'),
    ...viewTab(BROWSER, tabs.browser && !browser.pages.length, <Globe aria-hidden="true" />, 'Browser'),
  ];
  const items = [...views, ...pages, ...terms, ...files];
  const closeAll = (keep?: Item) => {
    close([...views, ...files].filter((i) => i !== keep).map((i) => i.key));
    for (const p of [...pages, ...terms]) if (p !== keep) p.close();
  };
  // The page and file tabs that fit, in the order opened, with the shown one always among them; the rest in the list.
  const shown = fitting(items, width, strip.current);
  const at = items.findIndex((i) => i.selected);
  const rest = items.length - shown.length;
  const find = (f: Opened) => (f.crumbs[0] === 'Designs' ? openTab(chat, f.path) : show(chat, f));
  const folders = tree ? [tree.project, tree.shared, ...tree.agents].filter(Boolean).length : 0;
  return (
    <>
      <div className="workspace-tabs">
        <div className="workspace-tablist" role="tablist" aria-label="Sidebar tabs">
          <div className="workspace-fixed">{tab(HOME, <House aria-hidden="true" />, 'Home')}</div>
          {items.length > 0 && <i className="workspace-sep" />}
          <div className="workspace-files" ref={strip}>
            {shown.map((i) => (
              <Tab
                key={i.key}
                id={id(i.key)}
                controls={i.key.startsWith('page:') ? id(BROWSER) : undefined}
                selected={i.selected}
                icon={i.icon}
                name={i.name}
                onSelect={i.select}
                onClose={i.close}
              />
            ))}
          </div>
          {rest > 0 && (
            <DropdownMenu>
              <DropdownMenuTrigger className="workspace-more">
                +{rest}
                <ChevronDown aria-hidden="true" />
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="workspace-more-list">
                {items.map((i) => (
                  <DropdownMenuItem key={i.key} data-on={i.selected || undefined} onSelect={i.select}>
                    {i.icon}
                    <span>{i.name}</span>
                    <small>{i.where}</small>
                  </DropdownMenuItem>
                ))}
                <DropdownMenuSeparator />
                <DropdownMenuItem onSelect={() => closeAll(items[at])}>Close others</DropdownMenuItem>
                <DropdownMenuItem onSelect={() => closeAll()}>Close all</DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          )}
        </div>
      </div>
      {view(
        HOME,
        <div className="home">
          <Find
            chat={chat}
            icon={<Search aria-hidden="true" />}
            label="Find file"
            project={tree?.project?.name}
            changes={changes}
            onOpen={find}
          >
            <h2>Open</h2>
            <div className="home-tiles">
              <button type="button" className="home-tile" onClick={() => go(FILES)}>
                <span className="home-tile-name">
                  <Folder aria-hidden="true" />
                  Files
                </span>
                <span className="home-tile-sub">
                  {tree && (folders ? count(folders, 'folder') : 'None yet')}
                </span>
              </button>
              <DesignsTile chat={chat} room={room} open={tabs.at === HOME} onOpen={() => go(DESIGN)} />
              <ChangesTile changes={changes} onOpen={() => go(CHANGES)} />
              <BrowserTile onOpen={() => go(BROWSER)} />
              <TerminalTile
                chat={chat}
                onOpen={() => {
                  const last = terminals.at(-1);
                  if (last) go(termKey(last.id));
                  else openTerminal(chat);
                }}
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
            icon={<Search aria-hidden="true" />}
            label="Filter files"
            project={tree?.project?.name}
            changes={changes}
            onOpen={find}
          >
            <Tree chat={chat} room={room} onOpen={(file) => show(chat, file)} />
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
      {tabs.changes && view(CHANGES, <Changes chat={chat} changes={changes} />)}
      {tabs.browser && view(BROWSER, <BrowserView open={tabs.at === BROWSER} />)}
      {terminals.map((t) =>
        view(
          termKey(t.id),
          <TerminalView
            info={t}
            by={t.agent ? nameOfAgent(t.agent) : undefined}
            open={tabs.at === termKey(t.id)}
            folders={starts}
            onLink={(url) => {
              say({ type: 'new', address: url });
              go(BROWSER);
            }}
          />,
        ),
      )}
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

/** Home's Designs tile: how many designs, and whether one is being written. */
function DesignsTile({
  chat,
  room,
  open,
  onOpen,
}: {
  chat: string;
  room: RoomState;
  open: boolean;
  onOpen: () => void;
}) {
  const { list, writer } = useDesigns(chat, room, open);
  const by = list?.map(writer).find((w) => w.writing)?.by;
  return (
    <button type="button" className="home-tile" onClick={onOpen}>
      <span className="home-tile-name">
        <DesignsMark />
        Designs
      </span>
      <span className="home-tile-sub">
        {list &&
          (list.length ? (by ? `${list.length} · writing` : count(list.length, 'design')) : 'None yet')}
      </span>
    </button>
  );
}

interface TabProps {
  id: number;
  /** The view it shows, when not its own: a page's tab shows the Browser view. */
  controls?: number;
  selected: boolean;
  icon: ReactNode;
  name: string;
  onSelect: () => void;
  /** A tab that closes, and shows its name; Home shows its icon only. */
  onClose?: () => void;
}

/** One tab: Home by its icon; a view, a page or a file by its name, with a close mark. */
function Tab({ id, controls, selected, icon, name, onSelect, onClose }: TabProps) {
  return (
    <div className="workspace-tab" data-selected={selected} data-file={!!onClose || undefined}>
      <button
        type="button"
        role="tab"
        id={`room-tab-${id}`}
        aria-label={onClose ? undefined : name}
        aria-selected={selected}
        aria-controls={`room-view-${controls ?? id}`}
        onClick={onSelect}
      >
        {icon}
        {onClose && <span>{name}</span>}
      </button>
      {onClose && (
        <IconButton aria-label={`Close ${name}`} className="workspace-close" onClick={onClose}>
          <X />
        </IconButton>
      )}
    </div>
  );
}
