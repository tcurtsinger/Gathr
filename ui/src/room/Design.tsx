import { Check, Columns2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure, type FileEntry, type Question } from '../api';
import { useCard } from '../components/Corner';
import { DesignsMark } from '../components/DesignsMark';
import { Face } from '../components/Face';
import { fileName, onWindows, pathSteps, split } from '../components/files';
import { IconButton } from '../components/IconButton';
import { StateMark } from '../components/StateMark';
import type { Opened } from './Files';
import { isPage, PageFrame } from './PageFrame';
import type { OnAnswer } from './Question';
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
const letter = (i: number) => String.fromCharCode(65 + i);

/**
 * The pages of the design in `folder` that a question asks the person to pick from, in its choices' order: when every
 * file it attaches is an HTML page right in that folder, none twice, one per choice. Else none, and it is an ordinary
 * question with files.
 */
function asked(q: Question, folder: string) {
  const pages = split(q.question).files;
  const keys = pages.map(key);
  const fits =
    pages.length === q.choices.length &&
    new Set(keys).size === keys.length &&
    keys.every((k) => isPage(k) && k.slice(0, k.lastIndexOf('/')) === key(folder));
  return pages.length && fits ? pages : undefined;
}

/** The question a design follows: the newest open one asking for a pick in it, else the newest answered. */
function following(room: RoomState, folder: string) {
  const asking = room.questions.filter((q) => asked(q, folder));
  return asking.findLast((q) => !q.answered) ?? asking.at(-1);
}

/** Which of its choices an answered question picked: -1 for none. */
const pickOf = (q?: Question) =>
  q?.answered && q.pick ? q.choices.findIndex((c) => c.label === q.pick) : -1;

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
        const q = following(room, d.path);
        const picked = pickOf(q);
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
            {w?.writing ? (
              <Writing />
            ) : q && !q.answered ? (
              <span className="room-state live design-mark">
                <StateMark state="needs" size={12} />
                Pick
              </span>
            ) : (
              picked >= 0 && (
                <span className="room-state design-mark">
                  <Check aria-hidden="true" />
                  {letter(picked)}
                </span>
              )
            )}
          </button>
        );
      })}
    </div>
  );
}

/** The tab's width from which a design's options can show two side by side, each still readable (the drawing's
 * two options sit in a 743px tab). */
const SIDE = 700;

interface Props {
  chat: string;
  design: Opened;
  room: RoomState;
  /** Shows the option on that page. */
  onShow: (page: string) => void;
  onAnswer: OnAnswer;
}

/**
 * A design's tab: its name, A, B, C for its options, and the option's page as it looks, drawn again each time an
 * agent's write in the folder ends. While a question asks for a pick in it, its options are the question's choices,
 * each with Pick; else its pages in their files' order. Side by side when the panel is wide enough.
 */
export function DesignTab({ chat, design, room, onShow, onAnswer }: Props) {
  const name = design.crumbs.at(-1) ?? design.path;
  const [pages, setPages] = useState<FileEntry[]>();
  const [side, setSide] = useState(false);
  const [wide, setWide] = useState(false);
  const [sending, setSending] = useState(false);
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  const tab = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!tab.current) return;
    const seen = new ResizeObserver(([e]) => setWide(e.contentRect.width >= SIDE));
    seen.observe(tab.current);
    return () => seen.disconnect();
  }, []);
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
  const q = following(room, design.path);
  const options = q
    ? (asked(q, design.path) ?? []).map((path, i) => ({
        path,
        name: fileName(path),
        label: q.choices[i].label,
      }))
    : (pages ?? []).map((p) => ({ path: p.path, name: p.name, label: undefined }));
  const shown = Math.max(
    0,
    options.findIndex((o) => key(o.path) === key(design.page ?? '')),
  );
  const picked = pickOf(q);
  const two = wide && side && options.length > 1;
  const pick = (i: number) => {
    if (!q || sending) return;
    setSending(true);
    onAnswer(q, q.choices[i].label).then(
      () => setSending(false),
      (f: Failure) => {
        setFailure(f);
        setSending(false);
      },
    );
  };
  // Pick while the question is open; Picked on the option it picked.
  const verdict = (i: number) =>
    q && !q.answered ? (
      <Button type="button" size="xs" disabled={sending} onClick={() => pick(i)}>
        Pick
      </Button>
    ) : (
      i === picked && (
        <span className="room-state design-picked">
          <Check aria-hidden="true" />
          Picked
        </span>
      )
    );
  const frame = (i: number) => (
    <PageFrame chat={chat} path={options[i].path} name={options[i].name} redraw={saves} />
  );
  const pair = shown + 1 < options.length ? [shown, shown + 1] : [shown - 1, shown];
  return (
    <div className="filetab" ref={tab}>
      <div className="filetab-path design-bar">
        <DesignsMark />
        <span>
          <strong>{name}</strong>
        </span>
        {options.length > 1 && (
          <fieldset className="plugins-kind design-options" aria-label="Options">
            {options.map((o, i) => (
              <button key={o.path} type="button" aria-pressed={i === shown} onClick={() => onShow(o.path)}>
                {i === picked && <Check aria-hidden="true" />}
                {letter(i)}
              </button>
            ))}
          </fieldset>
        )}
        {wide && options.length > 1 && (
          <IconButton aria-label="Side by side" aria-pressed={side} onClick={() => setSide(!side)}>
            <Columns2 />
          </IconButton>
        )}
        {w?.writing && <Writing />}
        {!two && options.length > 0 && verdict(shown)}
      </div>
      {two ? (
        <div className="design-side">
          {pair.map((i) => (
            <div key={options[i].path} className="design-option" data-shown={i === shown || undefined}>
              <div className="design-option-head">
                <b>{letter(i)}</b>
                <span>{options[i].label ?? options[i].name}</span>
                {verdict(i)}
              </div>
              {frame(i)}
            </div>
          ))}
        </div>
      ) : (
        options.length > 0 && frame(shown)
      )}
    </div>
  );
}
