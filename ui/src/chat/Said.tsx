import { MessagePrimitive } from '@assistant-ui/react';
import { Check } from 'lucide-react';
import { Fragment, type ReactNode } from 'react';
import { Face, type FaceSize, YouFace } from '../components/Face';
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
  keep,
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
  /** Its steps stay on its name line: a failed step, or an agent still working. */
  keep?: boolean;
  root?: boolean;
  children?: ReactNode;
}) {
  const props = {
    className: 'said',
    'data-agent': agent,
    'data-small': small || undefined,
    'data-you': you || undefined,
    'data-keep': keep || undefined,
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
 * task and its tick), and `end` after them: the steps.
 */
export function NameLine({ name, pieces, end }: { name: string; pieces: ReactNode[]; end?: ReactNode }) {
  const shown = pieces.filter((p) => p !== undefined && p !== null && p !== false && p !== '');
  return (
    <div className="name-line">
      <span className="name-line-name">{name}</span>
      <span className="name-line-meta">
        {shown.map((piece, i) => (
          // The pieces are in a fixed order: their place is their key.
          // biome-ignore lint/suspicious/noArrayIndexKey: see above
          <Fragment key={i}>
            {i > 0 && <Dot />}
            {piece}
          </Fragment>
        ))}
        {end}
      </span>
    </div>
  );
}

export const Dot = () => (
  <span className="name-line-dot" aria-hidden="true">
    ·
  </span>
);

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

/** The words, in a bubble: what it answers first, plainly, or the blue @Name of whom it is to. */
export function Bubble({
  quote,
  mention,
  children,
}: {
  quote?: string;
  mention?: string;
  children?: ReactNode;
}) {
  return (
    <div className="bubble">
      {quote && <div className="bubble-quote">{quote}</div>}
      {mention && <span className="mention">@{mention}</span>}
      {children}
    </div>
  );
}

/** Agents' thumbs-ups on the words above: the emoji and how many; who, on hover or focus, as names. */
export function Reactions({ names }: { names: string[] }) {
  return (
    <div className="reacts">
      <button type="button" className="react" aria-label={`Thumbs-up from ${list(names)}`}>
        <span className="react-e" aria-hidden="true">
          👍
        </span>
        <span aria-hidden="true">{names.length}</span>
        <span className="react-who" aria-hidden="true">
          {names.map((n) => (
            <span key={n}>{n}</span>
          ))}
        </span>
      </button>
    </div>
  );
}
