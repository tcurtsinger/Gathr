import { LoaderIcon } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import type { Member } from '../api';
import type { ChatState } from './state';
import { list, team } from './team';
import { began, waitingOn } from './waits';
import './team.css';

/** The members with a turn running on a hand-off: the ones the team column shows at work. */
export function working(state: ChatState): Member[] {
  const crew = team(state, new Map());
  const turns = Object.values(state.turns).filter((t) => t.status === 'running' && crew.rootOf(t.id));
  return state.members.filter((m) => turns.some((t) => t.agent === m.agent));
}

/**
 * What the Supervisor waits on while its round has open work, and since when (µs): its team by name, then
 * its own other open waits, as Summary words them (DESIGN.md). Nothing while it works itself.
 */
export function leadWait(state: ChatState) {
  const lead = state.members.find((m) => m.role === 'supervisor');
  if (!lead) return undefined;
  if (Object.values(state.turns).some((t) => t.agent === lead.agent && t.status === 'running'))
    return undefined;
  const { agents, other } = waitingOn(lead.agent, state);
  const team = [...agents].filter(([who]) => who && who !== lead.agent);
  if (team.length === 0) return undefined;
  const names = team.map(([who]) => state.members.find((m) => m.agent === who)?.name ?? 'an agent');
  const all = [...team, ...other];
  return { lead, names: [...names, ...other.keys()], since: Math.min(...all.map(([, us]) => us)) };
}

/** "Waiting on team" after the Supervisor's latest message; it opens the team column when that's hidden. */
export function TeamWait({ state, onOpen }: { state: ChatState; onOpen?: () => void }) {
  const wait = leadWait(state);
  if (!wait) return null;
  const tip = `${wait.lead.name} is waiting for ${list(wait.names)}, since ${began(wait.since)}`;
  const body = (
    <>
      <Spinner />
      Waiting on team
      <Initials members={working(state)} />
      <Elapsed since={wait.since} />
    </>
  );
  return (
    <TooltipProvider delayDuration={0}>
      <Tooltip>
        <TooltipTrigger asChild>
          {onOpen ? (
            <button type="button" className="working" onClick={onOpen}>
              {body}
            </button>
          ) : (
            <p className="working" role="status">
              {body}
            </p>
          )}
        </TooltipTrigger>
        <TooltipContent side="top">{tip}</TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}

/** "Working" at the foot of the team column, while members' turns run. */
export function TeamWorking({ state }: { state: ChatState }) {
  const busy = working(state);
  if (busy.length === 0) return null;
  return (
    <p className="working" role="status">
      <Spinner />
      Working
      <Initials members={busy} />
    </p>
  );
}

function Spinner() {
  return (
    <span className="working-spinner">
      <LoaderIcon
        aria-hidden="true"
        className="animate-spin [animation-duration:1.2s] motion-reduce:animate-none"
      />
    </span>
  );
}

function Initials({ members }: { members: Member[] }) {
  if (members.length === 0) return null;
  return (
    <span className="working-initials" role="img" aria-label={list(members.map((m) => m.name))}>
      {members.map((m) => (
        <span key={m.agent} aria-hidden="true">
          {initials(m.name)}
        </span>
      ))}
    </span>
  );
}

/** As the chat's avatars show them (speaker-identity): the first letters of the first two words. */
const initials = (name: string) =>
  name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => [...w][0]?.toUpperCase())
    .join('');

/** Time since the wait began (from its record), redrawn each minute: "· 38m", "· 2h 5m". */
function Elapsed({ since }: { since: number }) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const tick = setInterval(() => setNow(Date.now()), 60_000);
    return () => clearInterval(tick);
  }, []);
  const minutes = Math.max(1, Math.floor((now - since / 1000) / 60_000));
  const text = minutes < 60 ? `${minutes}m` : `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
  return <span className="working-time">· {text}</span>;
}
