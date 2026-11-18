import { Popover } from 'radix-ui';
import { type ReactNode, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { Own, Waiting } from '../api';
import { Face, GathrFace } from '../components/Face';
import { CANT_READ, cardParts, type Mark, since } from '../components/marks';
import { StateMark } from '../components/StateMark';
import type { Glance } from './glance';
import { gathrSaid, inWords, moving, parts } from './statusWords';
import './status-line.css';

interface Props {
  /** Every agent of the chat, the most pressing first. */
  crew: Glance[];
  /** The service's answer: which running turn is silent, and since when (0052). */
  waiting?: Waiting;
  /** Opens an agent's tab in the panel. */
  onOpenAgent: (agent: string) => void;
  /** Goes to what the person is needed for, when the chat holds it: "needs you" is then a button of its own. */
  onNeeds?: () => void;
}

/**
 * How the team stands, in one quiet line above the message box (the status-line design, A): who needs you or can't
 * run, then who works, is stopped, blocked or waits, with three dots while an agent works. Always the same slot, so
 * nothing moves when the line comes or goes. It opens a row for each agent that isn't free. What waits on the person
 * is said here and nowhere else above the box (the owner: "Keep the line"): a press on "needs you" goes to it.
 */
export function StatusLine({ crew, waiting, onOpenAgent, onNeeds }: Props) {
  const busy = crew.filter((g) => g.standing !== 'free');
  const own = waiting?.gathr ?? [];
  // Gathr's own is said like an agent's, last (the status-line design, its row A).
  const gathr = gathrSaid(own);
  const team = [
    ...busy.map((g) => ({ name: g.member.name, state: g.standing, on: g.on })),
    ...(gathr ? [gathr] : []),
  ];
  const said = parts(team);
  const words = said.map((p) => p.text).join(', ');
  const [open, setOpen] = useState(false);
  // Opening an agent moves the keyboard to its tab: closing must not bring it back to the line.
  const went = useRef(false);
  const go = (agent: string) => {
    went.current = true;
    setOpen(false);
    onOpenAgent(agent);
  };
  // Gone with the last part, it isn't open when the line comes back.
  useEffect(() => {
    if (!words) setOpen(false);
  }, [words]);
  const silent = new Map(
    (waiting?.lines ?? []).flatMap((l) => (l.agent && l.silent_us ? [[l.agent, l.silent_us / 1000]] : [])),
  );
  const needs = onNeeds && said[0]?.tone === 'needs' ? said[0] : undefined;
  const listed = needs ? said.slice(1) : said;
  return (
    <div className="status-slot">
      {needs && (
        <button type="button" className="status-line" onClick={onNeeds}>
          <span className="status-words">
            <span data-tone="needs">{needs.text}</span>
          </span>
        </button>
      )}
      {listed.length > 0 && (
        <Popover.Root open={open} onOpenChange={setOpen}>
          <Popover.Trigger asChild>
            <button type="button" className="status-line" aria-haspopup="dialog">
              <span className="status-words">
                {listed.map((p, i) => (
                  <span key={p.tone} data-tone={p.tone}>
                    {i > 0 && ', '}
                    {p.text}
                  </span>
                ))}
              </span>
              {moving(team) && <Dots />}
            </button>
          </Popover.Trigger>
          <Popover.Portal>
            <Popover.Content
              className="status-list"
              side="top"
              align="start"
              sideOffset={6}
              alignOffset={-6}
              collisionPadding={16}
              role="dialog"
              aria-label="Team"
              onCloseAutoFocus={(e) => {
                if (went.current) e.preventDefault();
                went.current = false;
              }}
            >
              <Rows team={busy} own={own} silent={silent} onOpen={go} />
            </Popover.Content>
          </Popover.Portal>
        </Popover.Root>
      )}
      {/* Beside the button, not around it: a live region would read the control out each time it changed. */}
      <span className="visually-hidden" role="status">
        {words}
      </span>
    </div>
  );
}

/** Three still dots that a light passes along once per turn of the Working ring, started with the rings. */
function Dots() {
  useLayoutEffect(() => {
    for (const a of document.getAnimations()) {
      const name = (a as CSSAnimation).animationName;
      if (name === 'status-pass' || name === 'mark-turn') a.startTime = 0;
    }
  }, []);
  return (
    <span className="status-dots" aria-hidden="true">
      <i />
      <i />
      <i />
    </span>
  );
}

/**
 * One row for each agent: its face, its name, then its state in the mark's shape and word, what and how long, as
 * its face's card says it; a silent turn says for how long it has made no progress. Then one for each of Gathr's own,
 * in the same shape, with Gathr's mark; they open nothing. Counted each second.
 */
function Rows({
  team,
  own,
  silent,
  onOpen,
}: {
  team: Glance[];
  own: Own[];
  silent: Map<string, number>;
  onOpen: (agent: string) => void;
}) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const tick = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(tick);
  }, []);
  const agents = team.map((g) => {
    const { agent, name } = g.member;
    const quiet = silent.get(agent);
    const said =
      quiet === undefined
        ? cardParts(g.standing, [g.about, g.failing ? CANT_READ : undefined], g.since, now)
        : cardParts(g.standing, [g.about, `silent ${since(now - quiet)}`], undefined, now);
    return (
      <Row
        key={agent}
        face={<Face agent={{ id: agent, name }} size={24} />}
        name={name}
        state={g.standing}
        said={said}
        cut={!!g.about}
        onClick={() => onOpen(agent)}
      />
    );
  });
  const gathrs = own.map((o) => {
    const what = o.name ? `${o.name}: ${o.what}` : inWords(o.what);
    const began = o.since_us === null ? undefined : o.since_us / 1000;
    const said = cardParts(o.state, [what, o.failing ? CANT_READ : undefined], began, now);
    return (
      <Row
        key={`${o.kind}:${o.name}:${o.what}`}
        face={<GathrFace size={24} />}
        name="Gathr"
        state={o.state}
        said={said}
        cut
      />
    );
  });
  return [...agents, ...gathrs];
}

/**
 * One row: a face, a name, and what its card says, the state's word in its mark's shape and Can't read set apart. When
 * `cut`, the piece after the word is the what: the part cut short when the row is too narrow, so the time stays.
 */
function Row({
  face,
  name,
  state,
  said,
  cut,
  onClick,
}: {
  face: ReactNode;
  name: string;
  state: Mark;
  said: string[];
  cut?: boolean;
  onClick?: () => void;
}) {
  const [word, ...rest] = said;
  const inner = (
    <>
      {face}
      <span className="status-row-name">{name}</span>
      <span className="status-row-state">
        <span data-state={state}>
          <StateMark state={state} size={12} />
          {word}
        </span>
        {rest.map((piece, i) => (
          <span key={piece} className={cut && i === 0 ? 'status-what' : undefined}>
            {' · '}
            {piece === CANT_READ ? <span className="status-cant">{piece}</span> : piece}
          </span>
        ))}
      </span>
    </>
  );
  if (!onClick) return <div className="status-row">{inner}</div>;
  return (
    <button type="button" className="status-row" onClick={onClick}>
      {inner}
    </button>
  );
}
