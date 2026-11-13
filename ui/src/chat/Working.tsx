import { LoaderIcon } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { Member } from '../api';
import { duration } from './fold';
import { InitialStack } from './Initial';
import type { ChatState } from './state';
import { list } from './team';
import './team.css';

/** The agents with a turn running, and when the longest-running of those turns began (ms). */
export function working(state: ChatState): { members: Member[]; since?: number } | undefined {
  const turns = Object.values(state.turns).filter((t) => t.status === 'running');
  if (turns.length === 0) return undefined;
  const began = turns.map((t) => Date.parse(t.created ?? '')).filter((t) => !Number.isNaN(t));
  return {
    members: state.members.filter((m) => turns.some((t) => t.agent === m.agent)),
    since: began.length ? Math.min(...began) : undefined,
  };
}

/**
 * "Working" after the latest message while a turn runs: who is at work, and for how long (0037). In a
 * team chat it is a button that opens Summary (`summary` says whether it is open).
 */
export function Working({
  state,
  summary,
  onOpen,
}: {
  state: ChatState;
  summary?: boolean;
  onOpen?: (from: HTMLElement) => void;
}) {
  const busy = working(state);
  if (!busy) return null;
  const who = list(busy.members.map((m) => m.name));
  const line = (
    <>
      <span className="working-spinner">
        <LoaderIcon
          aria-hidden="true"
          className="animate-spin [animation-duration:1.2s] motion-reduce:animate-none"
        />
      </span>
      Working
      {busy.members.length > 0 && (
        <span role="img" aria-label={who}>
          <InitialStack agents={busy.members} />
        </span>
      )}
      {busy.since !== undefined && (
        // Not read out: the status is who is working, and this is redrawn as time passes.
        <span className="working-time" aria-hidden="true">
          <Elapsed since={busy.since} to="minute" />
        </span>
      )}
    </>
  );
  if (!onOpen)
    return (
      <p className="working" role="status">
        {line}
      </p>
    );
  return (
    <div className="working">
      {/* Summary leaves a press here to this button, which opens or closes it. */}
      <button
        type="button"
        data-summary-opener=""
        aria-haspopup="dialog"
        aria-expanded={!!summary}
        onClick={(e) => onOpen(e.currentTarget)}
      >
        {line}
      </button>
      {/* Beside the button, not around it: a live region would read the control out each time it changed. */}
      <span className="visually-hidden" role="status">
        Working: {who}
      </span>
    </div>
  );
}

/**
 * How long something has run, redrawn as often as it changes: "· 4m 02s" each second, "· 12m" each
 * minute; `bare` leaves the dot out, for a time inside a sentence.
 */
export function Elapsed({
  since,
  to = 'second',
  lead = '',
  bare = false,
}: {
  since: number;
  to?: 'second' | 'minute';
  lead?: string;
  bare?: boolean;
}) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const tick = setInterval(() => setNow(Date.now()), to === 'minute' ? 60_000 : 1000);
    return () => clearInterval(tick);
  }, [to]);
  const took = duration(now - since, to);
  if (!took) return null;
  return bare ? took : `${lead}· ${took}`;
}
