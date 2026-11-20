import { Check, ChevronRight, ThumbsUp } from 'lucide-react';
import { type ReactNode, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { Member, Message, Reaction } from '../api';
import { type Approval, AskApproval } from '../components/Approval';
import { Face, type FaceMark, type FaceOf } from '../components/Face';
import { IconButton } from '../components/IconButton';
import { Markdown } from '../components/Markdown';
import { since } from '../components/marks';
import { StateMark } from '../components/StateMark';
import { QuestionCard } from './Question';
import { type Card, type Item, type Said, type Step, shape, stepsLine, working } from './shape';
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

/** What the person can do on a card: answer its approval, open its question, open a refusal's reference, like it. */
export interface Acts {
  approve: (request: string, allow: boolean) => void;
  /** Opens the questions panel on this question. */
  ask: (question: number) => void;
  reference: (ref: string) => void;
  like: (message: string, give: boolean) => void;
}

/** A search hit: the chat it opened and the message in it. */
export interface Found {
  chat: string;
  message: string;
}

interface Props {
  state: RoomState;
  people: People;
  acts: Acts;
  /** Scrolled to and marked briefly once it is drawn; a reply's thread opens for it. */
  found?: Found;
  /** Above the messages: a failure to load, with its Retry. */
  header?: ReactNode;
  /** At the foot: the message box. */
  children?: ReactNode;
}

/** The room: the person's messages and each agent's main message, replies to an agent tucked under it. */
export function Room({ state, people, acts, found, header, children }: Props) {
  const items = shape(state);
  const scroll = useStuck(state, found);
  const ctx = { people, reactions: state.reactions, approvals: state.approvals, acts, found: found?.message };
  return (
    <>
      <div className="room-scroll" ref={scroll}>
        <div className="room-col">
          {header}
          {items.map((item, i) => {
            const joined = !!items[i + 1] && items[i + 1].kind !== 'said';
            if (item.kind === 'said') return <You key={item.id} said={item} ctx={ctx} />;
            if (item.kind === 'card') return <Main key={item.id} card={item} joined={joined} ctx={ctx} />;
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
        </div>
      </div>
      <div className="room-foot">{children}</div>
    </>
  );
}

interface Ctx {
  people: People;
  reactions: Reaction[];
  approvals: Approval[];
  acts: Acts;
  /** The message a search hit opened the chat at. */
  found?: string;
}

/** Whether the item's words hold the message: `data-found` for it. */
function holds(item: Item, message?: string) {
  const has = item.kind === 'said' ? item.message.id === message : item.said.some((m) => m.id === message);
  return (!!message && has) || undefined;
}

/** The person's message: a bubble at the right, with its replies under it when it has any. */
function You({ said, ctx }: { said: Said; ctx: Ctx }) {
  return (
    <div className="room-item">
      <div className="room-you" data-found={holds(said, ctx.found)}>
        <Picked message={said.message} />
        <span className="room-time"> {clock(said.message.time)}</span>
      </div>
      <Reactions said={[said.message]} ctx={ctx} />
      {said.replies.length > 0 && <Thread replies={said.replies} ctx={ctx} />}
    </div>
  );
}

/** The person's words, each name they picked from the "@" list in blue. */
function Picked({ message }: { message: Message }) {
  const text = message.body as string;
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

/** An agent's main message: its picture with its state in the margin, the card, then its replies folded. */
function Main({ card, joined, ctx }: { card: Card; joined: boolean; ctx: Ctx }) {
  const agent = card.turn.agent as string;
  return (
    <div className="room-item room-turn">
      <span className="room-who">
        <Face agent={ctx.people.face(agent)} size={28} mark={markOf(card)} />
      </span>
      {joined && <span className="room-rule" aria-hidden="true" />}
      <article
        className="room-card"
        data-live={working(card) || undefined}
        data-found={holds(card, ctx.found)}
        aria-label={ctx.people.name(agent)}
      >
        <header className="room-head">
          <b>{ctx.people.name(agent)}</b>
          {ctx.people.lead(agent) && <span className="room-tag">Lead</span>}
          <span className="room-grow" />
          <State card={card} />
        </header>
        {card.text && <Markdown className="room-body" text={card.text} names={ctx.people.names} />}
        {!card.text && card.turn.why && <p className="room-body room-why">{card.turn.why}</p>}
        {card.steps.length > 0 && <Steps steps={card.steps} running={working(card)} />}
        <Foot said={card.said} last={card.last} ctx={ctx} />
        <Asks turn={card.id} agent={agent} ctx={ctx} />
      </article>
      {card.replies.length > 0 && <Thread replies={card.replies} ctx={ctx} />}
    </div>
  );
}

function markOf(card: Card): FaceMark {
  switch (card.turn.status) {
    case 'running':
    case 'queued':
      return { state: 'working', spin: true };
    case 'done':
      return { state: 'free' };
    case 'stopped':
      return { state: 'stopped' };
    default:
      return { state: 'cant' };
  }
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

/** The thumbs-ups on a message's words, as a small row under it: "👍 2 Ann, Bo". */
function Reactions({ said, ctx }: { said: Message[]; ctx: Ctx }) {
  const ids = new Set(said.map((m) => m.id));
  const given = ctx.reactions.filter((r) => ids.has(r.message));
  if (!given.length) return null;
  const who = [...new Set(given.map((r) => (r.agent === PERSON ? 'You' : ctx.people.name(r.agent))))];
  return (
    <div className="room-reactions">
      <span className="room-react">
        <span aria-hidden="true">👍</span> {given.length} <span className="room-time">{who.join(', ')}</span>
      </span>
    </div>
  );
}

/** An agent's foot: its thumbs-ups, and at the right the person's own, on its last words. */
function Foot({ said, last, ctx }: { said: Message[]; last?: Message; ctx: Ctx }) {
  if (!last) return <Reactions said={said} ctx={ctx} />;
  const mine = ctx.reactions.some((r) => r.message === last.id && r.agent === PERSON);
  return (
    <div className="room-foot-row">
      <Reactions said={said} ctx={ctx} />
      <IconButton
        aria-label="Like"
        aria-pressed={mine}
        className="room-like"
        onClick={() => ctx.acts.like(last.id, !mine)}
      >
        <ThumbsUp />
      </IconButton>
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
  if (item.kind === 'said')
    return (
      <div className="room-reply mine" data-found={holds(item, ctx.found)}>
        <span />
        <div>
          <div className="room-line">
            <b>You</b> <span className="room-time">{clock(item.message.time)}</span>
          </div>
          <p>
            <Picked message={item.message} />
          </p>
          <Reactions said={[item.message]} ctx={ctx} />
        </div>
      </div>
    );
  const agent = item.turn.agent as string;
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
        {item.text && <Markdown className="room-said" text={item.text} names={ctx.people.names} />}
        <Foot said={item.said} last={item.last} ctx={ctx} />
        <Asks turn={item.id} agent={agent} ctx={ctx} />
      </div>
    </div>
  );
}

/** "9:21", in the person's own clock. */
export function clock(iso?: string | null) {
  return iso ? new Date(iso).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }) : '';
}

/** Now, read again every 15 seconds while something runs. */
function useNow(running: boolean) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!running) return;
    const timer = setInterval(() => setNow(Date.now()), 15_000);
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
  const open = state.questions.filter((q) => !q.answered).length;
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
