import { useEffect, useState } from 'react';
import { api, type Failure, type FileEntry } from '../api';
import { useCard } from '../components/Corner';
import { DesignsMark } from '../components/DesignsMark';
import { Face } from '../components/Face';
import { onWindows, pathSteps } from '../components/files';
import { StateMark } from '../components/StateMark';
import type { Opened } from './Files';
import { isPage, PageFrame } from './PageFrame';
import type { RoomState } from './useRoom';
import './design.css';

/** A design: a folder in the project's part of Designs holding HTML pages, one per option. */
interface Listed {
  name: string;
  path: string;
  pages: number;
  /** When its newest page changed. */
  modified: number;
}

/** What the chat's steps say of one design: who wrote in it last, whether a running turn writes in it, how many of
 * its writes have ended. */
interface Written {
  agent?: string;
  writing: boolean;
  saves: number;
}

/** The steps that write a file and name it: Claude Code's Write and Edit, and Codex's file change, shown as Edit. */
const WRITES = ['Write', 'Edit'];

/** A folder's path the one way, to compare: its case ignored on Windows. */
const key = (path: string) => {
  const joined = pathSteps(path).join('/');
  return onWindows(path) ? joined.toLowerCase() : joined;
};

/**
 * The designs in `designs` the chat's steps wrote in, by their folder's `key`. Only a step that names a full path is
 * seen; Codex names the first file of a change only ("… +1 more").
 */
function written(room: RoomState, designs: string) {
  const base = key(designs);
  const out = new Map<string, Written>();
  for (const m of room.messages) {
    const body = m.kind === 'tool' && typeof m.body === 'object' && 'tool' in m.body ? m.body : undefined;
    const path =
      body && WRITES.includes(body.tool) ? body.input?.text.replace(/ \+\d+ more$/, '') : undefined;
    if (!body || !path || !(onWindows(path) || path.startsWith('/'))) continue;
    const at = key(path);
    if (!at.startsWith(`${base}/`)) continue;
    const rest = at.slice(base.length + 1).split('/');
    if (rest.length < 2) continue;
    const folder = `${base}/${rest[0]}`;
    const turn = m.turn ? room.turns[m.turn] : undefined;
    const was = out.get(folder) ?? { writing: false, saves: 0 };
    out.set(folder, {
      agent: turn?.agent ?? was.agent,
      writing: was.writing || turn?.status === 'running',
      saves: was.saves + (body.status === 'running' ? 0 : 1),
    });
  }
  return out;
}

const pagesOf = (entries: FileEntry[]) => entries.filter((e) => !e.folder && isPage(e.name));

/** "Writing" while an agent's running turn writes in the design. */
function Writing() {
  return (
    <span className="room-state live design-writing">
      <StateMark state="working" size={12} spin />
      Writing
    </span>
  );
}

/** The Designs tab: the chat's designs, newest first, each opening in its own tab. */
export function Designs({
  chat,
  room,
  onOpen,
}: {
  chat: string;
  room: RoomState;
  onOpen: (d: Listed) => void;
}) {
  const [designs, setDesigns] = useState<string | null>();
  const [list, setList] = useState<Listed[]>();
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  // The project's Designs is made when its first engine starts: until it is there, each step looks again.
  const lookAgain = designs === null ? room.messages.filter((m) => m.kind === 'tool').length : 0;
  // biome-ignore lint/correctness/useExhaustiveDependencies: each step looks again while there is no Designs
  useEffect(() => {
    api.tree(chat).then((t) => setDesigns(t.designs?.path ?? null), setFailure);
  }, [chat, lookAgain]);
  const seen = designs ? written(room, designs) : new Map<string, Written>();
  const saves = [...seen.values()].reduce((n, w) => n + w.saves, 0);
  // biome-ignore lint/correctness/useExhaustiveDependencies: each write that ends reads the list again
  useEffect(() => {
    if (designs === undefined) return;
    if (designs === null) return setList([]);
    let live = true;
    const read = async () => {
      const folders = (await api.folder(chat, designs)).filter((e) => e.folder);
      const each = await Promise.all(
        folders.map(async (d) => {
          const pages = pagesOf(await api.folder(chat, d.path));
          const modified = Math.max(0, ...pages.map((p) => p.modified ?? 0));
          return { name: d.name, path: d.path, pages: pages.length, modified };
        }),
      );
      return each.filter((d) => d.pages > 0).sort((a, b) => b.modified - a.modified);
    };
    read().then(
      (l) => live && setList(l),
      (f: Failure) => live && setFailure(f),
    );
    return () => {
      live = false;
    };
  }, [chat, designs, saves]);
  if (!list) return null;
  if (!list.length)
    return (
      <div className="filetab-none">
        <DesignsMark />
        <strong>No designs</strong>
        <span>Ask an agent to draw one.</span>
      </div>
    );
  return (
    <div className="design-list">
      {list.map((d) => {
        const w = seen.get(key(d.path));
        const by = room.members.find((m) => m.agent === w?.agent);
        return (
          <button key={d.path} type="button" className="design-row" onClick={() => onOpen(d)}>
            <span>
              <span className="design-name">{d.name}</span>
              <span className="board-meta">
                {by && <Face agent={{ id: by.agent, name: by.name }} size={16} />}
                {by && `${by.name} · `}
                {d.pages === 1 ? '1 page' : `${d.pages} options`}
              </span>
            </span>
            {w?.writing && <Writing />}
          </button>
        );
      })}
    </div>
  );
}

const letter = (i: number) => String.fromCharCode(65 + i);

/** A design's tab: its name, A, B, C for its options in their files' order, and the option's page as it looks,
 * drawn again each time an agent's write in the folder ends. */
export function DesignTab({ chat, design, room }: { chat: string; design: Opened; room: RoomState }) {
  const name = design.crumbs.at(-1) ?? design.path;
  const [pages, setPages] = useState<FileEntry[]>();
  const [at, setAt] = useState(0);
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  const w = written(room, design.path.replace(/[\\/][^\\/]*$/, '')).get(key(design.path));
  const saves = w?.saves ?? 0;
  // biome-ignore lint/correctness/useExhaustiveDependencies: each write that ends reads the options again
  useEffect(() => {
    let live = true;
    api.folder(chat, design.path).then(
      (e) => live && setPages(pagesOf(e)),
      (f: Failure) => live && setFailure(f),
    );
    return () => {
      live = false;
    };
  }, [chat, design.path, saves]);
  const shown = Math.min(at, (pages?.length ?? 1) - 1);
  const page = pages?.[shown];
  return (
    <div className="filetab">
      <div className="filetab-path design-bar">
        <DesignsMark />
        <span>
          <strong>{name}</strong>
        </span>
        {pages && pages.length > 1 && (
          <fieldset className="plugins-kind design-options" aria-label="Options">
            {pages.map((p, i) => (
              <button key={p.path} type="button" aria-pressed={i === shown} onClick={() => setAt(i)}>
                {letter(i)}
              </button>
            ))}
          </fieldset>
        )}
        {w?.writing && <Writing />}
      </div>
      {page && <PageFrame chat={chat} path={page.path} name={page.name} redraw={saves} />}
    </div>
  );
}
