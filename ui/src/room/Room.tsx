import { BookOpen, Check, ChevronRight, Copy, Plug, ThumbsUp } from 'lucide-react';
import { type ReactNode, useContext, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Member, type Message, type Reaction } from '../api';
import { type Approval, AskApproval } from '../components/Approval';
import { DesignsMark } from '../components/DesignsMark';
import { Face, type FaceOf } from '../components/Face';
import { PathTiles } from '../components/FileTile';
import { fileName, OpenFile, parts, split } from '../components/files';
import { IconButton } from '../components/IconButton';
import { Markdown } from '../components/Markdown';
import { MemoryMark } from '../components/MemoryMark';
import { since } from '../components/marks';
import { ScheduledMark } from '../components/ScheduledMark';
import { StateMark } from '../components/StateMark';
import { listen } from '../live';
import { OpenMemory } from '../memory/Memory';
import { OpenPlugin, type ToolNext } from '../plugins/Plugins';
import { OpenScheduled } from '../scheduled/Scheduled';
import { FilesCard } from './FilesCard';
import { type Made, notesOf, readNote } from './note';
import { QuestionCard, waits } from './Question';
import { Shown } from './Shown';
import {
  type Card,
  type Item,
  keptOf,
  type Needs,
  type Said,
  type Step,
  shape,
  stepsLine,
  working,
} from './shape';
import type { RoomState } from './useRoom';
import './room.css';

/** The person as a giver of a thumbs-up: the service's `person`. */
const PERSON = 'person';

/** Who is who in the room: each agent's name, picture and whether it is the lead. */
export interface People {
  name: (agent: string) => string;
  face: (agent: string) => FaceOf;
  lead: (agent: string) => boolean;
  /** The names an @Name in an agent's words is drawn blue for. */
  names: string[];
}

export function people(members: Member[], known: FaceOf[]): People {
  const byId = new Map(known.map((a) => [a.id, a]));
  const name = (id: string) => members.find((m) => m.agent === id)?.name ?? byId.get(id)?.name ?? 'An agent';
  return {
    name,
    face: (id) => byId.get(id) ?? { id, name: name(id) },
    lead: (id) => members.some((m) => m.agent === id && m.role === 'supervisor'),
    names: members.map((m) => m.name),
  };
}

/** What the person can do on a card: answer its approval, open its question, like it. */
export interface Acts {
  approve: (request: string, allow: boolean) => void;
  /** Opens the questions panel on this question. */
  ask: (question: number) => void;
  /** Unset in an archived chat, which reads only. */
  like?: (message: string, give: boolean) => void;
  /** Opens the chat's Changes tab. */
  changes?: () => void;
}

/** A search hit: the chat it opened and the message in it. */
export interface Found {
  chat: string;
  message: string;
}

interface Props {
  /** The chat: where its pictures are served from. */
  chat: string;
  /** Its project: what a note about the project opens in Memory. */
  project?: string | null;
  state: RoomState;
  people: People;
  acts: Acts;
  /** Scrolled to and marked briefly once it is drawn; a reply's thread opens for it. */
  found?: Found;
  /** Above the messages: a new chat's greeting. */
  header?: ReactNode;
  /** After the messages: the person's messages that weren't sent. */
  after?: ReactNode;
  /** At the foot: the message box. */
  children?: ReactNode;
}

/** The room: the person's messages and each agent's main message, replies to an agent tucked under it. */
export function Room({ chat, project, state, people, acts, found, header, after, children }: Props) {
  const items = shape(state);
  const scroll = useStuck(state, found);
  const ctx = {
    chat,
    project,
    people,
    reactions: state.reactions,
    approvals: state.approvals,
    acts,
    found: found?.message,
    notes: notesOf(state.messages, state.turns),
  };
  return (
    <>
      <div className="room-scroll" ref={scroll}>
        <div className="room-col">
          {header}
          {items.map((item, i) => {
            const next = items[i + 1]?.kind;
            const joined = next === 'card' || next === 'question';
            if (item.kind === 'said') return <You key={item.id} said={item} ctx={ctx} />;
            if (item.kind === 'card') return <Main key={item.id} card={item} joined={joined} ctx={ctx} />;
            if (item.kind === 'line')
              return (
                <p key={item.id} className="room-quiet">
                  {item.agent && <Face agent={people.face(item.agent)} size={16} />}
                  {item.agent && <b>{people.name(item.agent)}</b>} {item.text}
                </p>
              );
            return (
              <QuestionCard
                key={item.id}
                question={item.question}
                people={people}
                onOpen={acts.ask}
                joined={joined}
              />
            );
          })}
          {after}
        </div>
      </div>
      <div className="room-foot">{children}</div>
    </>
  );
}

interface Ctx {
  chat: string;
  project?: string | null;
  people: People;
  reactions: Reaction[];
  approvals: Approval[];
  acts: Acts;
  /** The message a search hit opened the chat at. */
  found?: string;
  /** The chat's notes on designs: a message that begins with one draws it above its words. */
  notes: Made[];
}

/** Whether the item's words hold the message: `data-found` for it. */
function holds(item: Item, message?: string) {
  const has = item.kind === 'said' ? item.message.id === message : item.said.some((m) => m.id === message);
  return (!!message && has) || undefined;
}

/**
 * The person's message: a bubble at the right, its files' tiles under the words, with its replies under it when it has
 * any. Files alone sit without a bubble. Its bar is under it.
 */
function You({ said, ctx }: { said: Said; ctx: Ctx }) {
  const { words: all, files } = split(said.message.body as string);
  const made = ctx.notes.find((n) => n.id === said.message.id);
  const words = made ? (readNote(all)?.words ?? '') : all;
  const tiles = files.length > 0 && <PathTiles chat={ctx.chat} paths={files} />;
  return (
    <div className="room-item">
      {words || made ? (
        <div className="room-you" data-found={holds(said, ctx.found)}>
          {made && <NoteHead made={made} />}
          <Picked message={said.message} words={words} />
          {tiles}
        </div>
      ) : (
        <div className="room-you-files" data-found={holds(said, ctx.found)}>
          {tiles}
        </div>
      )}
      <Bar
        time={said.message.time}
        scheduled={said.message.scheduled}
        from={said.message.from}
        text={words}
        said={[said.message]}
        ctx={ctx}
      />
      {said.replies.length > 0 && <Thread replies={said.replies} ctx={ctx} />}
    </div>
  );
}

interface WordsProps {
  text: string;
  className: string;
  chat: string;
  names: string[];
  /** Whose words: a file link that isn't a full path is in its folder. */
  agent: string;
}

/** An agent's words: markdown, with each picture or file it names as a tile where it stands. */
export function Words({ text, className, chat, names, agent }: WordsProps) {
  const open = useContext(OpenFile);
  const pieces = parts(text);
  const words = pieces.every((p) => 'words' in p) ? (
    <Markdown className={className} text={text} names={names} />
  ) : (
    <div className={`${className} room-pieces`}>
      {pieces.map((p, i) =>
        'words' in p ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: a message's pieces keep their order
          <Markdown key={i} text={p.words} names={names} />
        ) : (
          // biome-ignore lint/suspicious/noArrayIndexKey: a message's pieces keep their order
          <PathTiles key={i} chat={chat} paths={p.files} />
        ),
      )}
    </div>
  );
  if (!open) return words;
  return <OpenFile.Provider value={(path) => open(path, agent)}>{words}</OpenFile.Provider>;
}

/** A note's first line, drawn quietly above the person's words: its pin's number, the design and option, the part. */
function NoteHead({ made: { n, note } }: { made: Made }) {
  const design = fileName(note.page.replace(/[\\/][^\\/]*$/, ''));
  return (
    <span className="room-note">
      <span className="room-note-where">
        <b>{n}</b>
        <DesignsMark />
        {design} · {note.letter}
      </span>
      <span className="room-note-part">
        The {note.kind}
        {note.words && ` “${note.words}”`}
      </span>
    </span>
  );
}

/** The person's words, each name they picked from the "@" list in blue. */
function Picked({ message, words: text }: { message: Message; words: string }) {
  const picks = [...(message.picks ?? [])].sort((a, b) => a.at - b.at);
  const out: ReactNode[] = [];
  let from = 0;
  for (const p of picks) {
    out.push(text.slice(from, p.at));
    out.push(
      <span key={p.at} className="mention">
        {text.slice(p.at, p.at + p.len)}
      </span>,
    );
    from = p.at + p.len;
  }
  out.push(text.slice(from));
  return <span className="room-words">{out}</span>;
}

/** An agent's main message: its picture in the margin, the card, then its replies folded. */
function Main({ card, joined, ctx }: { card: Card; joined: boolean; ctx: Ctx }) {
  // A turn sent to no agent has none: it is drawn as "An agent".
  const agent = card.turn.agent ?? '';
  const running = working(card);
  // Once the turn ends, the files card is the record of the steps that changed a file: the line holds the rest.
  const rest = running ? card.steps : card.steps.filter((s) => 'note' in s || !s.changed?.length);
  return (
    <div className="room-item room-turn">
      <span className="room-who">
        <Face agent={ctx.people.face(agent)} size={28} />
      </span>
      {joined && <span className="room-rule" aria-hidden="true" />}
      <article
        className="room-card"
        data-live={running || undefined}
        data-found={holds(card, ctx.found)}
        aria-label={ctx.people.name(agent)}
      >
        <header className="room-head">
          <b>{ctx.people.name(agent)}</b>
          {ctx.people.lead(agent) && <span className="room-tag">Lead</span>}
          <span className="room-grow" />
          <State card={card} />
        </header>
        {card.text && (
          <Words
            className="room-body"
            text={card.text}
            chat={ctx.chat}
            names={ctx.people.names}
            agent={agent}
          />
        )}
        {!card.text && card.turn.why && <p className="room-body room-why">{card.turn.why}</p>}
        <Shown chat={ctx.chat} blocks={card.blocks} />
        {!running && <FilesCard steps={card.steps} agent={agent} onView={ctx.acts.changes} />}
        <Kept card={card} ctx={ctx} />
        {rest.length > 0 && <Steps steps={rest} running={running} />}
        <Asks turn={card.id} agent={agent} ctx={ctx} />
      </article>
      {!running && (
        <Bar
          time={card.turn.ended ?? card.turn.created}
          text={card.text ?? ''}
          said={card.said}
          last={card.last}
          ctx={ctx}
        />
      )}
      {card.replies.length > 0 && <Thread replies={card.replies} ctx={ctx} />}
    </div>
  );
}

/** At the head's right: "Working · 2m" while it runs, a check and how long it took once done. */
function State({ card }: { card: Card }) {
  const running = working(card);
  const now = useNow(running);
  const { status, created, ended } = card.turn;
  const began = created ? Date.parse(created) : now;
  if (running)
    return (
      <span className="room-state live">
        <StateMark state="working" size={12} spin />
        Working · {since(now - began)}
      </span>
    );
  if (status !== 'done')
    return (
      <span className="room-state">
        {status === 'stopped' ? 'Stopped' : status === 'waiting_sign_in' ? 'Waiting for sign-in' : 'Failed'}
      </span>
    );
  return (
    <span className="room-state">
      <Check aria-hidden="true" />
      {ended && created ? since(Date.parse(ended) - began, true) : ''}
    </span>
  );
}

/** All of the turn's tool runs and working notes as one line; a click opens them, in the order they came. */
function Steps({ steps, running }: { steps: Step[]; running: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="room-steps">
      <button type="button" className="room-work" aria-expanded={open} onClick={() => setOpen(!open)}>
        <ChevronRight aria-hidden="true" />
        {stepsLine(steps, running)}
      </button>
      {open && (
        <ol className="room-step-list">
          {steps.map((s, i) =>
            'note' in s ? (
              // biome-ignore lint/suspicious/noArrayIndexKey: a turn's steps only grow, in order
              <li key={i} className="room-note">
                {s.note}
              </li>
            ) : (
              // biome-ignore lint/suspicious/noArrayIndexKey: a turn's steps only grow, in order
              <li key={i} data-status={s.status}>
                <span>{s.input?.title ?? s.tool}</span>
                {s.input?.text && <code>{s.input.text}</code>}
              </li>
            ),
          )}
        </ol>
      )}
    </div>
  );
}

/** What a tool's line asks of the person: its button, the button once it's given, and where the tool's page opens. */
const ASKS: Record<Needs['what'], { next: ToolNext; button: string; given: string }> = {
  key: { next: 'key', button: 'Add', given: 'Added' },
  'sign-in': { next: 'sign-in', button: 'Sign in', given: 'Signed in' },
  value: { next: 'change', button: 'Add', given: 'Added' },
};

/** What an added tool needs, in a word or two: "needs key", "needs NOTES_KEY", "needs 4 values". */
function needsWords(needs: Needs) {
  if (needs.what !== 'value') return needs.what === 'key' ? 'needs key' : 'needs sign-in';
  const names = needs.names.split(' and ');
  return names.length === 1 ? `needs ${names[0]}` : `needs ${names.length} values`;
}

/**
 * Whether the tool still needs what its line said: asked of the tool as the line shows and again when the tools
 * change. `given` once it has it, `gone` once the tool is removed; as its line said until the tool answers, and
 * while it's disabled.
 */
function useNeeds(id: string, needs?: Needs['what']): 'needs' | 'given' | 'gone' {
  const [now, setNow] = useState<'needs' | 'given' | 'gone'>('needs');
  useEffect(() => {
    if (!needs) return;
    let live = true;
    const ask = () =>
      api.tool(id).then(
        (t) => {
          const still = {
            key: t.state === 'needs_key',
            'sign-in': t.state === 'sign_in',
            value: !!t.empty?.length,
          };
          if (live) setNow(!t.enabled || still[needs] ? 'needs' : 'given');
        },
        () => live && setNow('gone'),
      );
    ask();
    const stop = listen((event) => {
      if (event.type === 'tools' || event.type === 'lagged') ask();
    });
    return () => {
      live = false;
      stop();
    };
  }, [id, needs]);
  return now;
}

/** "Added tool Notes", on one row; what it needs from the person, with a button that reads Added once it has it. */
function ToolLine({ tool }: { tool: Extract<ReturnType<typeof keptOf>[number], { kind: 'tool' }> }) {
  const openPlugin = useContext(OpenPlugin);
  const now = useNeeds(tool.id, tool.needs?.what);
  const asks = tool.needs && now !== 'gone' && ASKS[tool.needs.what];
  return (
    <span className="room-kept">
      <button
        type="button"
        className="room-kept-line room-kept-row"
        onClick={() => openPlugin?.({ tool: tool.id })}
      >
        <Plug aria-hidden="true" />
        <span>Added tool</span> <b className="room-kept-cut">{tool.name}</b>
        {tool.needs && now === 'needs' && <span className="room-kept-cut">· {needsWords(tool.needs)}</span>}
      </button>
      {asks && (
        <Button
          type="button"
          size="xs"
          variant="outline"
          disabled={now === 'given'}
          onClick={() => openPlugin?.({ tool: tool.id, next: asks.next })}
        >
          {now === 'given' ? asks.given : asks.button}
        </Button>
      )}
    </span>
  );
}

/** A note line's label as shown, from the stored one: "Noted for own work" is "Noted", "Changed a note …" "Changed note …". */
function noteWords(label: string) {
  return label.replace(/ for own work$/, '').replace(/^Changed a note/, 'Changed note');
}

/**
 * What the turn kept that lasts, a quiet line each under its words: "Noted about you: …" opens Memory there, "Added
 * tool Notes" and "Added skill branch-names" open its page in Plugins, "Scheduled weekdays at 9:00: …" opens it in
 * Scheduled.
 */
function Kept({ card, ctx }: { card: Card; ctx: Ctx }) {
  const openMemory = useContext(OpenMemory);
  const openPlugin = useContext(OpenPlugin);
  const openScheduled = useContext(OpenScheduled);
  const kept = keptOf(card.steps);
  if (!kept.length) return null;
  // A tool's note names its place; the others are known by their label.
  const place = (k: Extract<(typeof kept)[number], { kind: 'note' }>) => {
    if (k.place) return k.place;
    if (k.label === 'Noted about you' || k.label === 'Changed a note about you') return 'you';
    if (k.label.endsWith(' own work')) return `agent:${card.turn.agent}`;
    return `project:${ctx.project}`;
  };
  return (
    <div className="room-noted">
      {kept.map((k, i) => {
        // biome-ignore lint/suspicious/noArrayIndexKey: a turn's lines only grow, in order
        if (k.kind === 'tool') return <ToolLine key={i} tool={k} />;
        if (k.kind === 'scheduled')
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: as above
            <span key={i} className="room-kept">
              <button type="button" className="room-kept-line" onClick={() => openScheduled?.(k.id)}>
                <ScheduledMark />
                <span className="room-kept-when">{k.label}:</span> <b>{k.text}</b>
              </button>
            </span>
          );
        if (k.kind === 'skill')
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: as above
            <span key={i} className="room-kept">
              <button
                type="button"
                className="room-kept-line room-kept-row"
                onClick={() => openPlugin?.({ skill: k.name })}
              >
                <BookOpen aria-hidden="true" />
                <span>{k.label.startsWith('Changed') ? 'Changed skill' : 'Added skill'}</span>{' '}
                <b className="room-kept-cut">{k.name}</b>
              </button>
            </span>
          );
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: as above
          <span key={i} className="room-kept">
            <button type="button" className="room-kept-line" onClick={() => openMemory?.(place(k))}>
              <MemoryMark />
              <span>{noteWords(k.label)}:</span> <b>{k.text}</b>
            </button>
          </span>
        );
      })}
    </div>
  );
}

interface BarProps {
  /** Left out in a thread, where the time is beside the name. */
  time?: string | null;
  /** A scheduled run's words: when they were due, said in place of the time. */
  scheduled?: string;
  /** A project's brief: the assistant that wrote it, said with its face in place of the time. */
  from?: string;
  /** The words Copy takes; none, no Copy. */
  text: string;
  /** The messages whose thumbs-ups are counted. */
  said: Message[];
  /** An agent's last words: what the person's own thumbs-up goes on. Unset on the person's own message. */
  last?: Message;
  ctx: Ctx;
}

/**
 * The thin row under a message (the owner's pick, 2026-11-22): its time, Copy, then the one thumbs-up with its number
 * last, so a growing number moves nothing. The thumb is filled once the person gave theirs, and who gave one shows
 * while the pointer is on it. On the person's own message the thumb is there only once an agent gave one.
 */
function Bar({ time, scheduled, from, text, said, last, ctx }: BarProps) {
  const [copied, setCopied] = useState(false);
  const ids = new Set(said.map((m) => m.id));
  const given = ctx.reactions.filter((r) => ids.has(r.message));
  const who = [...new Set(given.map((r) => (r.agent === PERSON ? 'You' : ctx.people.name(r.agent))))];
  const mine = given.some((r) => r.agent === PERSON);
  const like = ctx.acts.like;
  const press = last && like ? () => like(last.id, !mine) : undefined;
  const copy = () =>
    navigator.clipboard.writeText(text).then(
      () => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      },
      () => undefined,
    );
  if (!time && !text && !press && !given.length) return null;
  return (
    <div className="room-bar">
      {from ? (
        <span className="room-time room-from">
          <Face agent={ctx.people.face(from)} size={16} />
          From {ctx.people.name(from)}
        </span>
      ) : scheduled ? (
        <span className="room-time room-scheduled">
          <ScheduledMark />
          Scheduled · {clock(scheduled)}
        </span>
      ) : (
        time && <span className="room-time">{clock(time)}</span>
      )}
      {text && (
        <IconButton aria-label="Copy" onClick={copy}>
          {copied ? <Check className="room-bar-icon" /> : <Copy className="room-bar-icon" />}
        </IconButton>
      )}
      {(press || given.length > 0) && (
        <span className="room-thumb" data-mine={mine || undefined}>
          {press ? (
            <IconButton aria-label="Like" aria-pressed={mine} onClick={press}>
              <ThumbsUp className="room-bar-icon" />
            </IconButton>
          ) : (
            <ThumbsUp className="room-bar-icon room-thumb-still" aria-hidden="true" />
          )}
          {given.length > 0 && <b>{given.length}</b>}
          {who.length > 0 && <span className="room-tip">{who.join(', ')}</span>}
        </span>
      )}
    </div>
  );
}

/** The approvals a turn waits on, at the foot of its card. */
function Asks({ turn, agent, ctx }: { turn: string; agent: string; ctx: Ctx }) {
  const asks = ctx.approvals.filter((a) => a.turn === turn);
  if (!asks.length) return null;
  return (
    <div className="room-asks">
      {asks.map((a) => (
        <AskApproval key={a.request} approval={a} agent={ctx.people.name(agent)} actions={ctx.acts} />
      ))}
    </div>
  );
}

/** Replies to a message, tucked under it and folded to one line: "3 replies · Rev, Ann · 9:21". */
function Thread({ replies, ctx }: { replies: Item[]; ctx: Ctx }) {
  const [opened, setOpen] = useState(false);
  // A search hit on one of the replies opens them.
  const [seen, setSeen] = useState<string>();
  if (ctx.found !== seen) {
    setSeen(ctx.found);
    if (replies.some((r) => holds(r, ctx.found))) setOpen(true);
  }
  // A reply that waits for an approval is never folded away.
  const open = opened || replies.some((r) => ctx.approvals.some((a) => a.turn === r.id));
  const authors = [
    ...new Set(replies.map((r) => (r.kind === 'said' ? 'You' : ctx.people.name(r.turn.agent ?? '')))),
  ];
  const last = replies.at(-1) as Item;
  const n = replies.length;
  return (
    <div className="room-thread">
      <button type="button" className="room-fold" aria-expanded={open} onClick={() => setOpen(!open)}>
        <b>{open ? 'Hide replies' : `${n} ${n === 1 ? 'reply' : 'replies'}`}</b>
        {!open && (
          <span>
            {authors.join(', ')} · {clock(last.kind === 'said' ? last.message.time : last.turn.created)}
          </span>
        )}
      </button>
      {open && (
        <div className="room-replies">
          {replies.map((r) => (
            <Reply key={r.id} item={r} ctx={ctx} />
          ))}
        </div>
      )}
    </div>
  );
}

/** One reply in a thread: a small picture, the name and time, the words. */
function Reply({ item, ctx }: { item: Item; ctx: Ctx }) {
  if (item.kind === 'said') {
    const { words, files } = split(item.message.body as string);
    return (
      <div className="room-reply mine" data-found={holds(item, ctx.found)}>
        <span />
        <div>
          <div className="room-line">
            <b>You</b> <span className="room-time">{clock(item.message.time)}</span>
          </div>
          {words && (
            <p>
              <Picked message={item.message} words={words} />
            </p>
          )}
          {files.length > 0 && <PathTiles chat={ctx.chat} paths={files} />}
          <Bar text={words} said={[item.message]} ctx={ctx} />
        </div>
      </div>
    );
  }
  const agent = item.turn.agent ?? '';
  const running = working(item);
  return (
    <div className="room-reply" data-found={holds(item, ctx.found)}>
      <Face agent={ctx.people.face(agent)} size={20} />
      <div>
        <div className="room-line">
          <b>{ctx.people.name(agent)}</b>{' '}
          {running ? (
            <span className="room-state live">Working</span>
          ) : (
            <span className="room-time">{clock(item.turn.created)}</span>
          )}
        </div>
        {item.text && (
          <Words
            className="room-said"
            text={item.text}
            chat={ctx.chat}
            names={ctx.people.names}
            agent={agent}
          />
        )}
        <Shown chat={ctx.chat} blocks={item.blocks} />
        {!running && <FilesCard steps={item.steps} agent={agent} onView={ctx.acts.changes} />}
        <Kept card={item} ctx={ctx} />
        {!running && <Bar text={item.text ?? ''} said={item.said} last={item.last} ctx={ctx} />}
        <Asks turn={item.id} agent={agent} ctx={ctx} />
      </div>
    </div>
  );
}

/** "9:21", in the person's own clock. */
export function clock(iso?: string | null) {
  return iso ? new Date(iso).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }) : '';
}

/** Now, read again every second while something runs: a turn's first minute counts in seconds. */
function useNow(running: boolean) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!running) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [running]);
  return running ? Math.max(now, Date.now()) : now;
}

/**
 * The chat stays at its newest message while the person is there; scrolled up, it stays where they are. A new question
 * to the person brings it down to its newest message, so it is seen. A search hit, once drawn, is put in the middle.
 */
function useStuck(state: RoomState, found?: Found) {
  const box = useRef<HTMLDivElement>(null);
  const stuck = useRef(true);
  const open = state.questions.filter(waits).length;
  const asked = useRef(open);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    let width = el.clientWidth;
    const read = () => {
      // A scroll the box's new width made, before the observer below hears of it, isn't the person's.
      if (el.clientWidth === width) stuck.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    };
    // The box resized: the window, or the sidebar opening beside it after the chat drew.
    const resized = new ResizeObserver(() => {
      width = el.clientWidth;
      if (stuck.current) el.scrollTop = el.scrollHeight;
    });
    resized.observe(el);
    el.addEventListener('scroll', read);
    return () => {
      el.removeEventListener('scroll', read);
      resized.disconnect();
    };
  }, []);
  const centred = useRef<Found>(undefined);
  // biome-ignore lint/correctness/useExhaustiveDependencies: every change to the room may grow it
  useLayoutEffect(() => {
    if (open > asked.current) stuck.current = true;
    asked.current = open;
    const el = box.current;
    const hit = found !== centred.current && el?.querySelector('[data-found]');
    if (hit) {
      centred.current = found;
      stuck.current = false;
      hit.scrollIntoView({ block: 'center' });
    } else if (el && stuck.current) el.scrollTop = el.scrollHeight;
  }, [state, found]);
  return box;
}
