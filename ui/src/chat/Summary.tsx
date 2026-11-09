import { useId, useRef, useState } from 'react';
import { SpeakerIdentity } from '@/components/assistant-ui/elements/speaker-identity';
import type { Member, Task, Wait } from '../api';
import type { ChatState } from './state';
import { clock, list, roleName } from './team';
import './summary.css';

const open = (t: Task) => !['done', 'blocked', 'failed', 'stopped', 'cancelled'].includes(t.state);

/**
 * Every agent of the chat at a glance: role, what it's doing or waiting for, and its tasks (DESIGN.md
 * "Summary"). The prototype's popover from the chat header; Escape or a click outside closes it.
 */
export function Summary({ state }: { state: ChatState }) {
  const id = useId();
  const panel = useRef<HTMLDivElement>(null);
  const [shown, setShown] = useState(false);
  const members = [...state.members].sort((a, b) => rank(a) - rank(b));
  // A null owner is the person: a question the Supervisor put to them.
  const name = (agent: string | null) =>
    agent === null ? 'you' : (state.members.find((m) => m.agent === agent)?.name ?? 'an agent');

  // 8px below the chat's header and 8px in from its corner, kept on screen.
  const place = () => {
    const el = panel.current;
    const header = el?.parentElement?.closest('header');
    if (!el || !header) return;
    const at = header.getBoundingClientRect();
    const size = el.getBoundingClientRect();
    el.style.left = `${Math.max(16, Math.min(at.right - size.width - 8, innerWidth - size.width - 16))}px`;
    el.style.top = `${Math.max(16, Math.min(at.bottom + 8, innerHeight - size.height - 16))}px`;
  };

  return (
    <>
      <button type="button" className="summary-open" popoverTarget={id} aria-expanded={shown}>
        Summary
      </button>
      <div
        ref={panel}
        id={id}
        popover="auto"
        className="summary"
        role="dialog"
        aria-label="Summary"
        onToggle={(e) => {
          setShown(e.newState === 'open');
          place();
        }}
      >
        <h3>Agents</h3>
        {members.map((m) => {
          const tasks = state.tasks.filter((t) => t.owner === m.agent);
          return (
            <details key={m.agent} className="summary-agent">
              <summary>
                <SpeakerIdentity
                  className="w-auto"
                  turns={[{ id: m.agent, kind: 'agent', name: m.name, text: '' }]}
                />
                <small>
                  {roleName[m.role]} · {doing(m, state, name)}
                </small>
              </summary>
              <ul>
                {tasks.length === 0 && <li>No assigned tasks</li>}
                {[...tasks.filter(open), ...tasks.filter((t) => !open(t))].map((t) => (
                  <li key={t.number}>
                    {t.what}
                    {!open(t) && <span className="summary-done"> · {t.state}</span>}
                  </li>
                ))}
              </ul>
            </details>
          );
        })}
      </div>
    </>
  );
}

function rank(m: Member) {
  return m.role === 'supervisor' ? 0 : 1;
}

/**
 * Working; out of usage; stopped; paused, or held when work waits for Resume; waiting (and for whom, by
 * name); or ready. From the service's records only.
 */
export function doing(m: Member, state: ChatState, name: (agent: string | null) => string): string {
  const turns = Object.values(state.turns).filter((t) => t.agent === m.agent);
  if (turns.some((t) => t.status === 'running')) return 'Working';
  if (m.usage_until_us) return `Out of usage until ${clock(new Date(m.usage_until_us / 1000).toISOString())}`;
  if (state.run.state === 'stopped') return 'Stopped';
  const mine = state.tasks.filter((t) => t.owner === m.agent && open(t));
  if (m.paused || state.run.state !== 'running')
    return mine.length || turns.some((t) => t.status === 'queued') ? 'Held' : 'Paused';
  const after = mine.flatMap((t) => t.after);
  const on = [
    ...state.waits.filter((w) => w.agent === m.agent && w.state === 'open').map((w) => waitName(w, name)),
    ...state.tasks.filter((t) => after.includes(t.number) && open(t)).map((t) => name(t.owner)),
    ...state.tasks.filter((t) => t.giver === m.agent && open(t)).map((t) => name(t.owner)),
  ];
  if (on.length) return `Waiting for ${list([...new Set(on)])}`;
  return mine.length || turns.some((t) => t.status === 'queued') ? 'Waiting' : 'Ready';
}

/** A wait in a few words, as the service names it (tasks/words.rs `wait_name`). */
function waitName(w: Wait, name: (agent: string) => string): string {
  const target = w.target ?? '';
  if (w.kind === 'agent') return name(target);
  if (w.kind === 'ci') return /^\d+$/.test(target) ? `CI on #${target}` : `CI on ${target.slice(0, 7)}`;
  if (w.kind === 'pr') return `#${target} to merge or close`;
  return 'a check';
}
