import { MessagePrimitive } from '@assistant-ui/react';
import { Check } from 'lucide-react';
import type { ReactNode } from 'react';
import { Face, type FaceSize, YouFace } from '../components/Face';
import { Reference } from '../components/Notice';
import { useMark } from './faceMarks';
import { useIsNew } from './reads';
import { clock, list } from './team';
import './said.css';
import './mains.css';

/**
 * One message beside its author's face, everyone on the left (0072): the face, then the name line and what was
 * said under it. `root` makes it an item of the chat's list. A main message's face hangs left of the column, so its words
 * are as wide as the message box (0075); a `reply` sits in its main message's thread, smaller, inside the thread's rule.
 */
export function Said({
  face,
  line,
  agent,
  small,
  you,
  gathr,
  bare,
  root,
  reply,
  children,
}: {
  face?: ReactNode;
  line?: ReactNode;
  /** Its author's id, for the page and the tests. */
  agent?: string;
  /** In a thread: a smaller face and words. */
  small?: boolean;
  you?: boolean;
  /** Gathr says it (0073): its bubble is outlined. */
  gathr?: boolean;
  /** Nothing under its name line (a failed Stop): its face sits beside it. */
  bare?: boolean;
  root?: boolean;
  reply?: boolean;
  children?: ReactNode;
}) {
  const props = {
    className: 'said',
    'data-agent': agent,
    'data-small': small || undefined,
    'data-you': you || undefined,
    'data-gathr': gathr || undefined,
    'data-bare': bare || undefined,
    'data-reply': reply || undefined,
  };
  const inner = (
    <>
      <div className="said-face">{face}</div>
      <div className="said-main">
        {line}
        {children}
      </div>
    </>
  );
  return root ? (
    <MessagePrimitive.Root {...props}>{inner}</MessagePrimitive.Root>
  ) : (
    <div {...props}>{inner}</div>
  );
}

/** An agent's face beside its message; in a team chat it opens the agent's panel. */
export function AgentFace({
  agent,
  name,
  message,
  size,
  onOpen,
}: {
  agent?: string;
  name: string;
  /** The message it heads: on the agent's latest, its Working mark turns (0071). */
  message?: string;
  size: FaceSize;
  onOpen?: () => void;
}) {
  const face = <Face agent={{ id: agent ?? '', name }} size={size} mark={useMark(agent, name, message)} />;
  if (!onOpen) return face;
  return (
    <button type="button" className="said-open" aria-label={`Open ${name}`} onClick={onOpen}>
      {face}
    </button>
  );
}

export const YouSaid = ({ small }: { small?: boolean }) => <YouFace size={small ? 28 : 36} />;

/**
 * The name line (the owner's call of 2026-11-18): the name, then the time and New with "·" between, and the person's
 * ask's size on their own message. In the agent's panel the task it worked on and its tick follow.
 */
export function NameLine({ name, pieces }: { name: string; pieces: ReactNode[] }) {
  // Each piece is its own element: the "·" before it is drawn by said.css, so a long chat makes no node for it.
  return (
    <div className="name-line">
      <span className="name-line-name">{name}</span>
      <span className="name-line-meta">{pieces}</span>
    </div>
  );
}

/** When it was said, as the clock reads. */
export const When = ({ time }: { time?: string }) =>
  time ? <time dateTime={time}>{clock(time)}</time> : null;

/** A reply said while the person was away (0077): "New" in the attention blue, until they come back again. */
export function NewWord({ topic, time }: { topic?: string; time?: string }) {
  return useIsNew(topic, time) ? <span className="name-line-new">New</span> : null;
}

/** In the agent's panel, the task a member's message works on, and a tick once it is done. */
export const OnTask = ({ what, done }: { what: string; done?: boolean }) => (
  <span className="name-line-task">
    <span className="name-line-what">on {what}</span>
    {done && <Check role="img" aria-label="Done" />}
  </span>
);

/** What the person can do about how a message ended, under its bubble: why, its reference, then the button. */
export function EndRow({
  why,
  reference,
  onReference,
  children,
}: {
  why?: string | null;
  reference?: string | null;
  onReference?: (ref: string) => void;
  children?: ReactNode;
}) {
  return (
    <div className="said-end">
      {why && <span>{why}</span>}
      {reference && onReference && <Reference reference={reference} onOpen={onReference} />}
      {children}
    </div>
  );
}

/**
 * The words, in a bubble: what it answers first, plainly, or the blue @Name of whom it is to; `lead` before both (the
 * steps a message between agents came after). `foot` is the row under it (`Foot`): bubble and row share one box as
 * wide as the wider of them, so the row's buttons end at the bubble's right edge, or after the row's own content when
 * the bubble is narrower (said.css).
 */
export function Bubble({
  lead,
  quote,
  onQuote,
  quoteLines,
  mention,
  foot,
  children,
}: {
  lead?: ReactNode;
  quote?: string;
  /** The quote leads to what it quotes: an ask's result to its ask. */
  onQuote?: () => void;
  /** The quote may take up to this many lines: Gathr's words that opened the turn (0078). */
  quoteLines?: number;
  mention?: string;
  foot?: ReactNode;
  children?: ReactNode;
}) {
  const bubble = (
    <div className="bubble">
      {lead}
      {quote &&
        (onQuote ? (
          <button type="button" className="bubble-quote" onClick={onQuote}>
            {quote}
          </button>
        ) : (
          <div className="bubble-quote" data-lines={quoteLines}>
            {quote}
          </div>
        ))}
      {mention && <span className="mention">@{mention}</span>}
      {children}
    </div>
  );
  if (!foot) return bubble;
  return (
    <div className="said-words">
      {bubble}
      {foot}
    </div>
  );
}

/**
 * Thumbs-ups on the words above: the emoji and how many; who, on hover or focus, as names, "You" first. With
 * onToggle, a click gives the person's like or takes it back (0057), and theirs is drawn in the attention blue.
 */
export function Reactions({
  names,
  mine,
  onToggle,
}: {
  names: string[];
  mine?: boolean;
  onToggle?: () => void;
}) {
  return (
    <div className="reacts">
      <button
        type="button"
        className="react"
        aria-label={`Thumbs-up from ${list(names)}`}
        aria-pressed={onToggle ? !!mine : undefined}
        data-mine={mine || undefined}
        onClick={onToggle}
      >
        <span className="react-e" aria-hidden="true">
          👍
        </span>
        <span aria-hidden="true">{names.length}</span>
      </button>
      {/* Beside the count, on its line; with no room there, under it: never over a message (said.css). */}
      <span className="react-who" aria-hidden="true">
        {names.map((n) => (
          <span key={n}>{n}</span>
        ))}
      </span>
    </div>
  );
}
