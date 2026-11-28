import {
  ChevronDown,
  Ellipsis,
  File as FileIcon,
  Folder,
  Globe,
  LoaderCircle,
  Monitor,
  Search,
  SquareTerminal,
  X,
} from 'lucide-react';
import { type CSSProperties, type ReactNode, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { api, type Changes as ChatChanges, type FileTree } from '../api';
import { BoardMark } from '../components/BoardMark';
import { ChangesMark } from '../components/ChangesMark';
import { DesignsMark } from '../components/DesignsMark';
import { Face } from '../components/Face';
import { pathSteps as steps, onWindows as windows } from '../components/files';
import { IconButton } from '../components/IconButton';
import { ResizeHandle } from '../components/ResizeHandle';
import { StateMark } from '../components/StateMark';
import { widest } from '../components/workspace';
import { BackgroundTasksMark, BackgroundView } from './Background';
import { Board } from './Board';
import { BrowserView } from './Browser';
import { Changes, useUnseen } from './Changes';
import { ComputerView } from './Computer';
import { Designs, DesignTab } from './Design';
import { FileTab, type Opened, Tree } from './Files';
import { Find } from './Find';
import { isPage } from './PageFrame';
import { onEmpty, pageName, say, site, useBrowser } from './pages';
import type { OnAnswer } from './Question';
import { folderName, type StartFolder, TerminalView } from './Terminal';
import { asked, openTerminal, tell, useTerminals, waiting } from './terminals';
import type { RoomState } from './useRoom';
import './panel.css';

/**
 * A chat's open pages, and the view shown: Task board, Files, Designs, Changes, Browser, or an opened path. The
 * browser's pages are the person's one browser's, the same in every chat: Browser shows the page it shows.
 */
interface Tabs {
  board: boolean;
  files: boolean;
  design: boolean;
  changes: boolean;
  browser: boolean;
  computer: boolean;
  background: boolean;
  open: Opened[];
  at: string;
}
const BOARD = 'board';
const FILES = 'files';
const DESIGN = 'design';
const CHANGES = 'changes';
const BROWSER = 'browser';
const COMPUTER = 'computer';
const BACKGROUND = 'background';
const termKey = (id: string) => `term:${id}`;
const FRESH: Tabs = {
  board: false,
  files: false,
  design: false,
  changes: false,
  browser: false,
  computer: false,
  background: false,
  open: [],
  at: BOARD,
};
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

onEmpty(() => {
  for (const [chat, tabs] of kept) {
    if (tabs.browser) change(chat, { ...tabs, browser: false, at: tabs.at === BROWSER ? BOARD : tabs.at });
  }
});

const VIEW_NAMES: Record<string, string> = {
  [BOARD]: 'Task board',
  [FILES]: 'Files',
  [DESIGN]: 'Designs',
  [CHANGES]: 'Changes',
  [BROWSER]: 'Browser',
  [COMPUTER]: 'Computer',
  [BACKGROUND]: 'Background',
};

function openView(chat: string, at: string) {
  const tabs = tabsOf(chat);
  change(chat, {
    ...tabs,
    board: tabs.board || at === BOARD,
    files: tabs.files || at === FILES,
    design: tabs.design || at === DESIGN,
    changes: tabs.changes || at === CHANGES,
    browser: tabs.browser || at === BROWSER,
    computer: tabs.computer || at === COMPUTER,
    background: tabs.background || at === BACKGROUND,
    at,
  });
}

/** Lives above the panel so external closes are seen while it is hidden. */
export function usePanelTabs(chat: string, open: boolean, onOpen: (open: boolean) => void) {
  const tabs = useSyncExternalStore(listen, () => tabsOf(chat));
  const browser = useBrowser();
  const terminals = useTerminals(chat).list;
  const keys = [
    ...Object.keys(VIEW_NAMES).filter((key) => tabs[key as keyof Tabs] === true),
    ...(browser.pages.length ? [BROWSER] : []),
    ...terminals.map((t) => termKey(t.id)),
    ...tabs.open.map((f) => f.path),
  ];
  useEffect(() => {
    if (!keys.length) {
      if (open && !asked.has(chat)) onOpen(false);
    } else if (!keys.includes(tabs.at)) openView(chat, keys[0]);
  });
  return () => {
    if (!open && !keys.length) openView(chat, BOARD);
    onOpen(!open);
  };
}

/** Direct view access, beside Agents and Needs you; available even when the panel is shut. */
export function PanelShortcuts({
  chat,
  changes,
  open,
  onOpen,
}: {
  chat: string;
  changes?: ChatChanges;
  open: boolean;
  onOpen: () => void;
}) {
  const tabs = useSyncExternalStore(listen, () => tabsOf(chat));
  const terminals = useTerminals(chat).list;
  const unseen = useUnseen(chat, changes, open && tabs.at === CHANGES);
  const selected = (key: string) =>
    open && (key === 'terminal' ? tabs.at.startsWith('term:') : tabs.at === key);
  const choose = (key: string) => {
    onOpen();
    if (key === 'terminal') {
      const last = terminals.at(-1);
      if (last) openTerminalTab(chat, last.id);
      else openTerminal(chat);
    } else openView(chat, key);
  };
  const shortcuts = [
    { key: BOARD, name: 'Task board', icon: <BoardMark /> },
    { key: FILES, name: 'Files', icon: <Folder /> },
    { key: 'terminal', name: 'Terminal', icon: <SquareTerminal /> },
    { key: BROWSER, name: 'Browser', icon: <Globe /> },
    { key: DESIGN, name: 'Designs', icon: <DesignsMark /> },
  ];
  const more = [
    { key: CHANGES, name: 'Changes', icon: <ChangesMark /> },
    { key: COMPUTER, name: 'Computer', icon: <Monitor /> },
    { key: BACKGROUND, name: 'Background', icon: <BackgroundTasksMark /> },
  ];
  return (
    <>
      {shortcuts.map((s) => (
        <IconButton
          key={s.key}
          aria-label={s.name}
          aria-pressed={selected(s.key)}
          className="head-button"
          onClick={() => choose(s.key)}
        >
          {s.icon}
        </IconButton>
      ))}
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <IconButton
            aria-label="More panels"
            className="head-button"
            data-on={more.some((s) => selected(s.key)) || undefined}
          >
            <Ellipsis />
            {unseen && <span className="head-dot" />}
          </IconButton>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          {more.map((s) => (
            <DropdownMenuItem
              key={s.key}
              data-on={selected(s.key) || undefined}
              onSelect={() => choose(s.key)}
            >
              {s.icon}
              {s.name}
              {s.key === CHANGES && unseen && <span className="panel-change-dot" />}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  );
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

/** Shows one of the chat's terminals in its tab, as a step that ran in it asks. */
export const openTerminalTab = (chat: string, id: string) =>
  change(chat, { ...tabsOf(chat), at: termKey(id) });

/** Shows the chat's Changes view. */
export const openChanges = (chat: string) => change(chat, { ...tabsOf(chat), changes: true, at: CHANGES });

/** Shows the chat's Background view. */
export const openBackground = (chat: string) =>
  change(chat, { ...tabsOf(chat), background: true, at: BACKGROUND });

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

/** The panel beside the chat: shortcuts open views and pages as closable tabs. */
export function RoomSidebar(props: PanelProps) {
  return (
    <aside className="workspace" aria-label="Chat sidebar">
      <ResizeHandle label="Resize chat sidebar" side="left" name="--workspace-width" min={360} max={widest} />
      <Panel key={props.chat} {...props} />
    </aside>
  );
}

interface PanelProps {
  changes?: ChatChanges;
  chat: string;
  room: RoomState;
  onAnswer: OnAnswer;
  /** Sends a note on a design, as the chat's own send does. */
  onNote: (text: string, agent?: string) => void;
}

/** A named tab's widest and narrowest, the gap between tabs, and the "+N" list's button with a two-digit count. */
const WIDEST = 180;
const NARROWEST = 84;
const GAP = 3;
const MORE = 54;
/** A tab's width beside its name: padding, icon and gap; the shown tab's close mark adds `CLOSE`. */
const AROUND = 43;
const CLOSE = 15;
/** A terminal's mark and the gap before it. */
const MARK = 16;

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
  /** A terminal's state after its name: running, or ended with a failure. */
  mark?: ReactNode;
  /** What its close mark does instead of closing: an agent's running terminal asks first. */
  ask?: () => void;
}

let ruler: CanvasRenderingContext2D | null | undefined;
/** How wide `name` is in a tab, by the strip's own font. */
function nameWidth(name: string, strip: HTMLElement | null) {
  ruler ??= document.createElement('canvas').getContext('2d');
  if (!ruler || !strip) return name.length * 7;
  ruler.font = `13px ${getComputedStyle(strip).fontFamily}`;
  return ruler.measureText(name).width;
}

/**
 * The tabs shown in `width`, in order, the shown one always among them, and the widest each may be. Tabs that
 * don't all fit as wide as their names share the room evenly, a short one keeping its own width, down to
 * `NARROWEST`; only then do the rest go in the "+N" list.
 */
function fitting(items: Item[], width: number, strip: HTMLElement | null) {
  const wide = items.map((i) =>
    Math.min(WIDEST, Math.ceil(nameWidth(i.name, strip)) + AROUND + (i.mark ? MARK : i.selected ? CLOSE : 0)),
  );
  // Each tab with the gap after it: the room is the strip's and one gap more.
  const least = (n: number) => Math.min(wide[n], NARROWEST) + GAP;
  let room = width + GAP;
  let shown = items.map((_, n) => n);
  if (shown.reduce((sum, n) => sum + least(n), 0) > room) {
    room -= MORE + GAP;
    const at = items.findIndex((i) => i.selected);
    let left = room - (at < 0 ? 0 : least(at));
    shown = shown.filter((n) => {
      if (n === at) return true;
      if (least(n) > left) left = Number.NEGATIVE_INFINITY;
      left -= least(n);
      return left >= 0;
    });
  }
  let fit = WIDEST;
  const sorted = shown.map((n) => wide[n] + GAP).sort((a, b) => a - b);
  for (const [j, w] of sorted.entries()) {
    const even = room / (sorted.length - j);
    if (w > even) {
      fit = Math.floor(even - GAP);
      break;
    }
    room -= w;
  }
  return { shown: shown.map((n) => items[n]), fit };
}

function Panel({ chat, room, changes, onAnswer, onNote }: PanelProps) {
  const tabs = useSyncExternalStore(listen, () => tabsOf(chat));
  const [tree, setTree] = useState<FileTree>();
  useEffect(() => {
    api.tree(chat).then(setTree, () => {});
  }, [chat]);
  const strip = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(Number.POSITIVE_INFINITY);
  // An agent's running terminal whose tab's close was pressed: it asks at its foot before it stops.
  const [asking, setAsking] = useState<string>();
  // The person's terminal with a command typed in for them, until they press a key in it.
  const [typedIn, setTypedIn] = useState<string>();
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
  const order = [
    BOARD,
    FILES,
    DESIGN,
    CHANGES,
    BROWSER,
    COMPUTER,
    BACKGROUND,
    ...browser.pages.map((p) => pageKey(p.id)),
  ];
  order.push(...terminals.map((t) => termKey(t.id)));
  order.push(...tabs.open.map((f) => f.path));
  const go = (at: string) => openView(chat, at);
  // A tab closes; when it was shown, the next tab shows, or the one before, or Task board.
  const close = (keys: string[]) => {
    const i = items.findIndex((t) => t.selected);
    const left = items.filter((t) => !keys.includes(t.key));
    const next =
      i >= 0 && keys.includes(items[i].key) ? (left[Math.min(i, left.length - 1)] ?? null) : undefined;
    if (next?.page) say({ type: 'show', page: next.page });
    change(chat, {
      ...tabs,
      board: tabs.board && !keys.includes(BOARD),
      files: tabs.files && !keys.includes(FILES),
      design: tabs.design && !keys.includes(DESIGN),
      changes: tabs.changes && !keys.includes(CHANGES),
      browser: tabs.browser && !keys.includes(BROWSER),
      computer: tabs.computer && !keys.includes(COMPUTER),
      background: tabs.background && !keys.includes(BACKGROUND),
      open: tabs.open.filter((f) => !keys.includes(f.path)),
      at: next === undefined ? tabs.at : next === null ? BOARD : next.page ? BROWSER : next.key,
    });
  };
  // The person's New shows the terminal it opens; one an agent starts waits for a look.
  const known = useRef<string[] | null>(null);
  useEffect(() => {
    const before = known.current;
    known.current = terminals.map((t) => t.id);
    if (!before) return;
    const fresh = terminals.find((t) => !t.agent && !before.includes(t.id));
    if (fresh && asked.delete(chat)) {
      go(termKey(fresh.id));
      if (waiting.delete(chat)) setTypedIn(fresh.id);
    } else if (tabs.at.startsWith('term:') && !terminals.some((t) => termKey(t.id) === tabs.at))
      change(chat, { ...tabsOf(chat), at: BOARD });
  });
  const id = (key: string) => order.indexOf(key);
  const view = (key: string, body: ReactNode) => (
    <section
      key={key}
      id={`room-view-${id(key)}`}
      role="tabpanel"
      aria-label={VIEW_NAMES[key]}
      aria-labelledby={VIEW_NAMES[key] ? undefined : `room-tab-${id(key)}`}
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
    mark: t.running ? (
      <StateMark state="working" size={9} spin />
    ) : t.code ? (
      <StateMark state="cant" size={9} />
    ) : undefined,
    selected: tabs.at === termKey(t.id),
    select: () => go(termKey(t.id)),
    close: () => {
      close([termKey(t.id)]);
      tell(chat, { type: 'close', id: t.id });
    },
    ask:
      t.agent && t.running
        ? () => {
            go(termKey(t.id));
            setAsking(t.id);
          }
        : undefined,
  }));
  const starts: StartFolder[] = [
    tree?.project ? { name: folderName(tree.project.path) } : { name: 'Home' },
    ...(tree?.agents ?? []).map((a) => ({ name: folderName(a.path), owner: a.name, path: a.path })),
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
  const viewTab = (key: string, on: boolean, icon: ReactNode): Item[] =>
    on
      ? [
          {
            key,
            icon,
            name: VIEW_NAMES[key],
            where: '',
            selected: tabs.at === key,
            select: () => go(key),
            close: () => close([key]),
          },
        ]
      : [];
  const views = [
    ...viewTab(BOARD, tabs.board, <BoardMark />),
    ...viewTab(FILES, tabs.files, <Folder />),
    ...viewTab(DESIGN, tabs.design, <DesignsMark />),
    ...viewTab(CHANGES, tabs.changes, <ChangesMark />),
    ...viewTab(BROWSER, tabs.browser && !browser.pages.length, <Globe />),
    ...viewTab(COMPUTER, tabs.computer, <Monitor />),
    ...viewTab(BACKGROUND, tabs.background, <BackgroundTasksMark />),
  ];
  const items = [...views, ...pages, ...terms, ...files];
  const closeAll = (keep?: Item) => {
    for (const p of [...pages, ...terms]) if (p !== keep) p.close();
    close(items.filter((i) => i !== keep).map((i) => i.key));
  };
  // The page and file tabs that fit, in the order opened, with the shown one always among them; the rest in the list.
  const { shown, fit } = fitting(items, width, strip.current);
  const at = items.findIndex((i) => i.selected);
  const rest = items.length - shown.length;
  const find = (f: Opened) => (f.crumbs[0] === 'Designs' ? openTab(chat, f.path) : show(chat, f));
  return (
    <>
      <div className="workspace-tabs" hidden={!items.length}>
        <div className="workspace-tablist" role="tablist" aria-label="Sidebar tabs">
          <div className="workspace-files" ref={strip} style={{ '--fit': `${fit}px` } as CSSProperties}>
            {shown.map((i) => (
              <Tab
                key={i.key}
                id={id(i.key)}
                controls={i.key.startsWith('page:') ? id(BROWSER) : undefined}
                selected={i.selected}
                icon={i.icon}
                name={i.name}
                mark={i.mark}
                onSelect={i.select}
                onClose={i.ask ?? i.close}
              />
            ))}
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
      </div>
      {tabs.board &&
        view(
          BOARD,
          <div className="board-view">
            <h2>Task board</h2>
            <Board chat={chat} />
          </div>,
        )}
      {tabs.files &&
        view(
          FILES,
          <Find
            chat={chat}
            icon={<Search aria-hidden="true" />}
            label="Find file"
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
      {tabs.computer && view(COMPUTER, <ComputerView chat={chat} nameOf={nameOfAgent} />)}
      {tabs.background &&
        view(
          BACKGROUND,
          <BackgroundView
            chat={chat}
            tasks={room.background}
            face={room.members.length > 1 ? nameOfAgent : undefined}
          />,
        )}
      {terminals.map((t) =>
        view(
          termKey(t.id),
          <TerminalView
            info={t}
            by={t.agent ? nameOfAgent(t.agent) : undefined}
            open={tabs.at === termKey(t.id)}
            folders={starts}
            typedIn={typedIn === t.id ? () => setTypedIn(undefined) : undefined}
            asking={
              asking === t.id
                ? {
                    stop: () => {
                      setAsking(undefined);
                      terms.find((i) => i.key === termKey(t.id))?.close();
                    },
                    keep: () => setAsking(undefined),
                  }
                : undefined
            }
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

interface TabProps {
  id: number;
  /** The view it shows, when not its own: a page's tab shows the Browser view. */
  controls?: number;
  selected: boolean;
  icon: ReactNode;
  name: string;
  mark?: ReactNode;
  onSelect: () => void;
  onClose: () => void;
}

/** One opened page, terminal, file or design, with its close mark. */
function Tab({ id, controls, selected, icon, name, mark, onSelect, onClose }: TabProps) {
  return (
    <div className="workspace-tab" data-selected={selected} data-file>
      <button
        type="button"
        role="tab"
        id={`room-tab-${id}`}
        aria-selected={selected}
        aria-controls={`room-view-${controls ?? id}`}
        onClick={onSelect}
      >
        {icon}
        <span>{name}</span>
        {mark}
      </button>
      <IconButton aria-label={`Close ${name}`} className="workspace-close" onClick={onClose}>
        <X />
      </IconButton>
    </div>
  );
}
