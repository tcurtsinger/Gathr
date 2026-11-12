import { LoaderIcon } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { Member } from '../api';
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

/** "Working" after the latest message while a turn runs: who is at work, and for how long (0037). */
export function Working({ state }: { state: ChatState }) {
  const busy = working(state);
  if (!busy) return null;
  return (
    <p className="working" role="status">
      <span className="working-spinner">
        <LoaderIcon
          aria-hidden="true"
          className="animate-spin [animation-duration:1.2s] motion-reduce:animate-none"
        />
      </span>
      Working
      <Initials members={busy.members} />
      {busy.since !== undefined && <Elapsed since={busy.since} />}
    </p>
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
export const initials = (name: string) =>
  name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => [...w][0]?.toUpperCase())
    .join('');

/** Time since it began, redrawn each minute: "· 38m", "· 2h 5m". */
function Elapsed({ since }: { since: number }) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const tick = setInterval(() => setNow(Date.now()), 60_000);
    return () => clearInterval(tick);
  }, []);
  const minutes = Math.max(1, Math.floor((now - since) / 60_000));
  const text = minutes < 60 ? `${minutes}m` : `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
  return <span className="working-time">· {text}</span>;
}
