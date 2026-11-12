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
 * team chat it is a button that opens Summary.
 */
export function Working({ state, onOpen }: { state: ChatState; onOpen?: (from: HTMLElement) => void }) {
  const busy = working(state);
  if (!busy) return null;
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
        <span role="img" aria-label={list(busy.members.map((m) => m.name))}>
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
  return (
    <p className="working" role="status">
      {onOpen ? (
        // Summary leaves a press here to this button, which opens or closes it.
        <button type="button" data-summary-opener="" onClick={(e) => onOpen(e.currentTarget)}>
          {line}
        </button>
      ) : (
        line
      )}
    </p>
  );
}

/** How long something has run, redrawn as often as it changes: "· 4m 02s" each second, "· 12m" each minute. */
export function Elapsed({
  since,
  to = 'second',
  lead = '',
}: {
  since: number;
  to?: 'second' | 'minute';
  lead?: string;
}) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const tick = setInterval(() => setNow(Date.now()), to === 'minute' ? 60_000 : 1000);
    return () => clearInterval(tick);
  }, [to]);
  const took = duration(now - since, to);
  return took ? `${lead}· ${took}` : null;
}
