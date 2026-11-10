import { Popover } from 'radix-ui';
import { SpeakerIdentity } from '@/components/assistant-ui/elements/speaker-identity';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import type { Member, PauseScope, Pauses, Task, Wait } from '../api';
import { ChatMenu } from './ChatMenu';
import { agentPause, type PauseAction, type Run } from './pauses';
import type { ChatState } from './state';
import { clock, list, roleName } from './team';
import './summary.css';

/** The prototype's own icon (ChatActions.tsx): two agents, each with a line. */
function SummaryIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <circle cx="6" cy="6.5" r="2.5" />
      <path d="M12 6.5h8" strokeLinecap="round" />
      <circle cx="6" cy="17.5" r="2.5" />
      <path d="M12 17.5h8" strokeLinecap="round" />
    </svg>
  );
}

const open = (t: Task) => !['done', 'blocked', 'failed', 'stopped', 'cancelled'].includes(t.state);

interface Props {
  state: ChatState;
  pauses: Pauses;
  /** How the chat runs: its own pause or the app's. */
  run: Run;
  onPause: (action: PauseAction, scope: PauseScope) => void;
}

/**
 * Every agent of the chat at a glance: role, what it's doing or waiting for, and its tasks (DESIGN.md
 * "Summary"), with Pause or Resume for each agent alone. The prototype's Radix popover from the chat
 * header, so an agent's menu opens above it; Escape or a click outside closes it.
 */
export function Summary({ state, pauses, run, onPause }: Props) {
  const members = [...state.members].sort((a, b) => rank(a) - rank(b));
  // A null owner is the person: a question the Supervisor put to them.
  const name = (agent: string | null) =>
    agent === null ? 'you' : (state.members.find((m) => m.agent === agent)?.name ?? 'an agent');

  return (
    <Popover.Root>
      <Popover.Trigger asChild>
        <TooltipIconButton tooltip="Summary">
          <SummaryIcon />
        </TooltipIconButton>
      </Popover.Trigger>
      <Popover.Anchor asChild>
        <span className="summary-anchor" aria-hidden="true" />
      </Popover.Anchor>
      <Popover.Portal>
        <Popover.Content
          className="summary"
          side="bottom"
          align="end"
          sideOffset={8}
          collisionPadding={16}
          role="dialog"
          aria-label="Summary"
        >
          <h3>Agents</h3>
          {members.map((m) => {
            const tasks = state.tasks.filter((t) => t.owner === m.agent);
            const own = agentPause(pauses, m.agent);
            const agent: PauseScope = { scope: 'agent', target: m.agent };
            return (
              <div key={m.agent} className="summary-agent">
                <details>
                  <summary>
                    <SpeakerIdentity
                      className="w-auto"
                      turns={[{ id: m.agent, kind: 'agent', name: m.name, text: '' }]}
                    />
                    <small>
                      {roleName[m.role]} · {doing(m, state, run, own, name)}
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
                <ChatMenu
                  label={`More for ${m.name}`}
                  pause={{
                    state: own.paused ? 'paused' : 'running',
                    onPause: () => onPause('pause', agent),
                    onResume: () => onPause('resume', agent),
                  }}
                />
              </div>
            );
          })}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

function rank(m: Member) {
  return m.role === 'supervisor' ? 0 : 1;
}

/**
 * Working; out of usage; stopped; paused, or held when work waits for Resume; waiting (and for whom, by
 * name); or ready. From the service's records only.
 */
function doing(
  m: Member,
  state: ChatState,
  run: Run,
  own: ReturnType<typeof agentPause>,
  name: (agent: string | null) => string,
): string {
  const turns = Object.values(state.turns).filter((t) => t.agent === m.agent);
  if (turns.some((t) => t.status === 'running')) return 'Working';
  if (own.usageUntil) return `Usage limit until ${clock(new Date(own.usageUntil / 1000).toISOString())}`;
  if (run.state === 'stopped') return 'Stopped';
  const mine = state.tasks.filter((t) => t.owner === m.agent && open(t));
  const queued = turns.some((t) => t.status === 'queued');
  if (own.paused || run.state !== 'running') return mine.length || queued ? 'Held' : 'Paused';
  const after = mine.flatMap((t) => t.after);
  const on = [
    ...state.waits.filter((w) => w.agent === m.agent && w.state === 'open').map((w) => waitName(w, name)),
    ...state.tasks.filter((t) => after.includes(t.number) && open(t)).map((t) => name(t.owner)),
    ...state.tasks.filter((t) => t.giver === m.agent && open(t)).map((t) => name(t.owner)),
  ];
  if (on.length) return `Waiting for ${list([...new Set(on)])}`;
  return mine.length || queued ? 'Waiting' : 'Ready';
}

/** A wait in a few words, as the service names it (tasks/words.rs `wait_name`). */
function waitName(w: Wait, name: (agent: string) => string): string {
  const target = w.target ?? '';
  if (w.kind === 'agent') return name(target);
  if (w.kind === 'ci') return /^\d+$/.test(target) ? `CI on #${target}` : `CI on ${target.slice(0, 7)}`;
  if (w.kind === 'pr') return `#${target} to merge or close`;
  return 'a check';
}
