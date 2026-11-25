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

/** What programs print in colour, per theme: green, cyan and the rest readable on Gathr's own background. */
const ANSI = {
  dark: {
    black: '#3a3a38',
    green: '#3fb950',
    yellow: '#d29922',
    magenta: '#c49cf0',
    cyan: '#56b6c2',
    white: '#c8c8c4',
    brightGreen: '#56d364',
    brightYellow: '#e3b341',
    brightMagenta: '#d2a8ff',
    brightCyan: '#76d9e3',
  },
  light: {
    black: '#1b1b1a',
    green: '#1a7f37',
    yellow: '#9a6700',
    magenta: '#8250df',
    cyan: '#0b7285',
    white: '#55554f',
    brightGreen: '#1a7f37',
    brightYellow: '#9a6700',
    brightMagenta: '#8250df',
    brightCyan: '#0b7285',
  },
};

/** The window's own colours, read again when its theme changes. */
function colours() {
  const root = document.documentElement;
  const css = getComputedStyle(root);
  const v = (name: string) => css.getPropertyValue(name).trim();
  return {
    ...ANSI[root.classList.contains('light') ? 'light' : 'dark'],
    background: v('--background'),
    foreground: v('--foreground'),
    cursor: v('--foreground'),
    cursorAccent: v('--background'),
    selectionBackground: 'rgba(128, 128, 128, 0.35)',
    red: v('--destructive'),
    brightRed: v('--destructive'),
    blue: v('--attention'),
    brightBlue: v('--attention'),
    brightBlack: v('--muted-foreground'),
    brightWhite: v('--foreground'),
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
  /** Its tab's close was pressed while an agent's command runs: Stop or Keep, at its foot. */
  asking?: { stop: () => void; keep: () => void };
}

/** One terminal at the tab's size: its folder, New and Stop over it, and a line at its foot once it ended. */
export function TerminalView({ info, by, open, folders, onLink, asking }: ViewProps) {
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
  const ended = info.code ? `Failed, code ${info.code}` : 'Ended';
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
          <div className="term-end" data-failed={!!info.code || undefined}>
            <span>{ended}</span>
            <button type="button" onClick={() => tell(chat, { type: 'again', id })}>
              Run again
            </button>
          </div>
        )}
        {info.running && asking && (
          <div className="term-end" data-asking="">
            <span>{`Stop ${info.name}? ${by} is using it.`}</span>
            <button type="button" onClick={asking.keep}>
              Keep
            </button>
            <button type="button" onClick={asking.stop}>
              Stop
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

/** Gathr's terminal tools as the steps list names them. */
const STEPS: Record<string, string> = {
  mcp__gathr__terminal_run: 'Ran in terminal',
  mcp__gathr__terminal_read: 'Read terminal',
  mcp__gathr__terminal_send: 'Typed in terminal',
  mcp__gathr__terminal_list: 'Listed terminals',
  mcp__gathr__ask_to_run: 'Asked you to run',
};
export const isTerminalStep = (tool: string) => tool in STEPS;

interface StepProps {
  chat: string;
  agent: string;
  tool: string;
  /** What the step kept: the command, or the terminal's id. */
  text?: string;
  onOpen?: (id: string) => void;
}

/** A terminal tool's step: what it did, then the command or the terminal by name, which opens its tab. */
export function TerminalStep({ chat, agent, tool, text, onOpen }: StepProps) {
  const { list } = useTerminals(chat);
  const ran = tool === 'mcp__gathr__terminal_run';
  const byId = tool === 'mcp__gathr__terminal_read' || tool === 'mcp__gathr__terminal_send';
  const found = ran
    ? list.findLast((t) => t.agent === agent && t.command === text)
    : byId
      ? list.find((t) => t.id === text)
      : undefined;
  // An id names nothing once its terminal is closed.
  const what = byId ? found?.name : text;
  return (
    <span className="room-step-name">
      <span>{STEPS[tool]}</span>
      {what &&
        (found && onOpen ? (
          <button type="button" className="room-step-term" onClick={() => onOpen(found.id)}>
            <code>{what}</code>
          </button>
        ) : (
          <code>{what}</code>
        ))}
    </span>
  );
}
