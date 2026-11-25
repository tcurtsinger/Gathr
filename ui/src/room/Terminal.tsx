import { FitAddon } from '@xterm/addon-fit';
import { type ILink, Terminal as Xterm } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';
import { Folder, Plus, Square, SquareTerminal } from 'lucide-react';
import { useEffect, useRef } from 'react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { IconButton } from '../components/IconButton';
import { follow, openTerminal, type TerminalInfo, tell, useTerminals } from './terminals';
import './terminal.css';

/** A folder a new terminal may start in, by the name the chat's tree gives it. */
export interface StartFolder {
  name: string;
  /** None for the chat's first: its project's, or the home folder. */
  path?: string;
}

/** Home's Terminal tile: how many of the chat's terminals run. */
export function TerminalTile({ chat, onOpen }: { chat: string; onOpen: () => void }) {
  const { list } = useTerminals(chat);
  const running = list.filter((t) => t.running).length;
  return (
    <button type="button" className="home-tile" onClick={onOpen}>
      <span className="home-tile-name">
        <SquareTerminal aria-hidden="true" />
        Terminal
      </span>
      <span className="home-tile-sub">{running ? `${running} running` : 'None'}</span>
    </button>
  );
}

const MONO = 'ui-monospace, "Cascadia Mono", Consolas, Menlo, monospace';
const URL_AT = /https?:\/\/[^\s"'<>`]+[^\s"'<>`.,;:!?)\]]/g;

/** The window's own colours, read again when its theme changes. */
function colours() {
  const css = getComputedStyle(document.documentElement);
  const v = (name: string) => css.getPropertyValue(name).trim();
  return {
    background: v('--background'),
    foreground: v('--foreground'),
    cursor: v('--foreground'),
    cursorAccent: v('--background'),
    selectionBackground: 'rgba(128, 128, 128, 0.35)',
  };
}

interface ViewProps {
  info: TerminalInfo;
  /** Who started it, for the folder line; none for the person's. */
  by?: string;
  open: boolean;
  folders: StartFolder[];
  /** An address in its output, clicked: it opens in Gathr's Browser. */
  onLink: (url: string) => void;
}

/** One terminal at the tab's size: its folder, New and Stop over it, and a line at its foot once it ended. */
export function TerminalView({ info, by, open, folders, onLink }: ViewProps) {
  const box = useRef<HTMLDivElement>(null);
  const term = useRef<Xterm | null>(null);
  const fit = useRef<FitAddon | null>(null);
  const link = useRef(onLink);
  link.current = onLink;
  const { chat, id } = info;

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const x = new Xterm({
      fontFamily: MONO,
      fontSize: 12.5,
      lineHeight: 1.2,
      scrollback: 10000,
      cursorBlink: true,
      theme: colours(),
    });
    const fitting = new FitAddon();
    x.loadAddon(fitting);
    x.open(el);
    term.current = x;
    fit.current = fitting;
    x.onData((data) => tell(chat, { type: 'input', id, data }));
    x.onResize(({ cols, rows }) => tell(chat, { type: 'resize', id, cols, rows }));
    x.registerLinkProvider({
      provideLinks(y, give) {
        const line = x.buffer.active.getLine(y - 1)?.translateToString(true) ?? '';
        const links: ILink[] = [...line.matchAll(URL_AT)].map((m) => ({
          text: m[0],
          range: { start: { x: (m.index ?? 0) + 1, y }, end: { x: (m.index ?? 0) + m[0].length, y } },
          activate: (_e, url) => link.current(url),
        }));
        give(links.length ? links : undefined);
      },
    });
    const unfollow = follow(chat, id, (bytes) => (bytes ? x.write(bytes) : x.reset()));
    const sized = new ResizeObserver(() => {
      if (el.offsetWidth && el.offsetHeight) fitting.fit();
    });
    sized.observe(el);
    const themed = new MutationObserver(() => {
      x.options.theme = colours();
    });
    themed.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
    return () => {
      unfollow();
      sized.disconnect();
      themed.disconnect();
      x.dispose();
      term.current = null;
    };
  }, [chat, id]);

  // Shown again, or run again: its size and the keys go to it.
  useEffect(() => {
    if (!open) return;
    fit.current?.fit();
    const x = term.current;
    if (x) tell(chat, { type: 'resize', id, cols: x.cols, rows: x.rows });
    if (info.running) x?.focus();
  }, [open, info.running, chat, id]);

  const name = info.folder.split(/[\\/]/).filter(Boolean).at(-1) ?? info.folder;
  const ended = info.code ? `Ended, code ${info.code}` : 'Ended';
  return (
    <div className="term">
      <div className="term-bar">
        <span className="term-folder">
          <Folder aria-hidden="true" />
          <span>{by ? `${by} · ${name}` : name}</span>
        </span>
        <NewTerminal chat={chat} folders={folders} />
        {info.running && (
          <IconButton aria-label="Stop" onClick={() => tell(chat, { type: 'stop', id })}>
            <Square />
          </IconButton>
        )}
      </div>
      <div className="term-body">
        <div className="term-screen" ref={box} />
        {!info.running && (
          <div className="term-end">
            <span>{ended}</span>
            <button type="button" onClick={() => tell(chat, { type: 'again', id })}>
              Run again
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

/** New: a terminal in the chat's folder, or, with more than one, a menu of them. */
function NewTerminal({ chat, folders }: { chat: string; folders: StartFolder[] }) {
  if (folders.length < 2)
    return (
      <IconButton aria-label="New terminal" onClick={() => openTerminal(chat)}>
        <Plus />
      </IconButton>
    );
  return (
    <DropdownMenu>
      <DropdownMenuTrigger aria-label="New terminal" className="term-new">
        <Plus aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {folders.map((f) => (
          <DropdownMenuItem key={f.path ?? ''} onSelect={() => openTerminal(chat, f.path)}>
            <Folder aria-hidden="true" />
            {f.name}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
