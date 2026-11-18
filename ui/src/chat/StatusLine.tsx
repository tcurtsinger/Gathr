import { Popover } from 'radix-ui';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { Waiting } from '../api';
import { Face } from '../components/Face';
import { cardLine, since, WORD } from '../components/marks';
import { StateMark } from '../components/StateMark';
import type { Glance } from './glance';
import { moving, parts } from './statusWords';
import './status-line.css';

interface Props {
  /** Every agent of the chat, the most pressing first. */
  crew: Glance[];
  /** The service's answer: which running turn is silent, and since when (0052). */
  waiting?: Waiting;
  /** Opens an agent's tab in the panel. */
  onOpenAgent: (agent: string) => void;
}

/**
 * How the team stands, in one quiet line above the message box (the status-line design, A): who needs you or can't
 * run, then who works, is stopped, blocked or waits, with three dots while an agent works. Always the same slot, so
 * nothing moves when the line comes or goes. It opens a row for each agent that isn't free.
 */
export function StatusLine({ crew, waiting, onOpenAgent }: Props) {
  const busy = crew.filter((g) => g.standing !== 'free');
  const team = busy.map((g) => ({ name: g.member.name, state: g.standing, on: g.on }));
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
  return (
    <div className="status-slot">
      {said.length > 0 && (
        <Popover.Root open={open} onOpenChange={setOpen}>
          <Popover.Trigger asChild>
            <button type="button" className="status-line" aria-haspopup="dialog">
              <span className="status-words">
                {said.map((p, i) => (
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
              <Rows team={busy} silent={silent} onOpen={go} />
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
 * its face's card says it; a silent turn says for how long it has made no progress. Counted each second.
 */
function Rows({
  team,
  silent,
  onOpen,
}: {
  team: Glance[];
  silent: Map<string, number>;
  onOpen: (agent: string) => void;
}) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const tick = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(tick);
  }, []);
  return team.map((g) => {
    const { agent, name } = g.member;
    const quiet = silent.get(agent);
    const line =
      quiet === undefined
        ? cardLine(g.standing, [g.about], g.since, now)
        : cardLine(g.standing, [g.about, `silent ${since(now - quiet)}`], undefined, now);
    return (
      <button type="button" key={agent} className="status-row" onClick={() => onOpen(agent)}>
        <Face agent={{ id: agent, name }} size={24} />
        <span className="status-row-name">{name}</span>
        <span className="status-row-state">
          <span data-state={g.standing}>
            <StateMark state={g.standing} size={12} />
            {WORD[g.standing]}
          </span>
          {line.slice(WORD[g.standing].length)}
        </span>
      </button>
    );
  });
}
