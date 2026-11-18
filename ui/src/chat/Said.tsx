import { MessagePrimitive } from '@assistant-ui/react';
import { Check } from 'lucide-react';
import type { ReactNode } from 'react';
import { Face, type FaceSize, YouFace } from '../components/Face';
import { Reference } from '../components/Notice';
import { useMark } from './faceMarks';
import { clock, list } from './team';
import './said.css';

/**
 * One message beside its author's face, everyone on the left (0072): the face, then the name line and what was
 * said under it. `root` makes it the thread's message itself; a message inside a thread item is not.
 */
export function Said({
  face,
  line,
  agent,
  small,
  you,
  gathr,
  keep,
  bare,
  root,
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
  /** Its steps stay on its name line: a failed step, or an agent still working. */
  keep?: boolean;
  /** It has no words yet: its face sits beside its name line. */
  bare?: boolean;
  root?: boolean;
  children?: ReactNode;
}) {
  const props = {
    className: 'said',
    'data-agent': agent,
    'data-small': small || undefined,
    'data-you': you || undefined,
    'data-gathr': gathr || undefined,
    'data-keep': keep || undefined,
    'data-bare': bare || undefined,
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
 * The name line: the name, then every other piece in one size and colour with "·" between (the role, the time, the
 * task and its tick, how the task or the turn ended), and `end` after them: the steps.
 */
export function NameLine({ name, pieces, end }: { name: string; pieces: ReactNode[]; end?: ReactNode }) {
  // Each piece is its own element: the "·" before it is drawn by said.css, so a long chat makes no node for it.
  return (
    <div className="name-line">
      <span className="name-line-name">{name}</span>
      <span className="name-line-meta">
        {pieces.map((p) => (typeof p === 'string' ? <span key={p}>{p}</span> : p))}
        {end}
      </span>
    </div>
  );
}

/** When it was said, as the clock reads. */
export const When = ({ time }: { time?: string }) =>
  time ? <time dateTime={time}>{clock(time)}</time> : null;

/** The task a member's message works on, and a tick once it is done. */
export const OnTask = ({ what, done }: { what: string; done?: boolean }) => (
  <span className="name-line-task">
    <span className="name-line-what">on {what}</span>
    {done && <Check role="img" aria-label="Done" />}
  </span>
);

/** How its turn ended, a word on the name line (0051): "Gathr restarted", "Needs plan"; `bad` in the danger colour. */
export const EndWord = ({ word, bad }: { word: string; bad?: boolean }) => (
  <span className="name-line-end" data-bad={bad || undefined}>
    {word}
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
 * The words, in a bubble: what it answers first, plainly, or the blue @Name of whom it is to. ctions is the row on
 * its corner (MessageActions).
 */
export function Bubble({
  quote,
  mention,
  actions,
  children,
}: {
  quote?: string;
  mention?: string;
  actions?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="bubble">
      {quote && <div className="bubble-quote">{quote}</div>}
      {mention && <span className="mention">@{mention}</span>}
      {children}
      {actions}
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
