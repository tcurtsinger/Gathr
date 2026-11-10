import { Popover } from 'radix-ui';
import { SpeakerIdentity } from '@/components/assistant-ui/elements/speaker-identity';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import type { Member, PauseScope, Pauses } from '../api';
import { ChatMenu } from './ChatMenu';
import { agentPause, type PauseAction, type Run } from './pauses';
import type { ChatState } from './state';
import { clock, list, roleName } from './team';
import { began, open, waitingOn } from './waits';
import './summary.css';

/** The prototype's own icon (ChatActions.tsx): two agents, each with a line. */
function SummaryIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden="true">
      <circle cx="6" cy="6.5" r="2.5" />
      <path d="M12 6.5h8" strokeLinecap="round" />
      <circle cx="6" cy="17.5" r="2.5" />
      <path d="M12 17.5h8" strokeLinecap="round" />
    </svg>
  );
}

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
        <TooltipIconButton tooltip="Summary" className="head-button">
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
 * Working; out of usage; stopped; paused, or held when work waits for Resume; waiting (for whom, by name,
 * and since when); or ready. From the service's records only.
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
  // Each name the agent waits for, with when its earliest wait on it began.
  const { agents, other } = waitingOn(m.agent, state);
  const since = [...[...agents].map(([who, us]) => [name(who), us] as const), ...other];
  if (since.length) return `Waiting for ${list(since.map(([who, us]) => `${who} since ${began(us)}`))}`;
  return mine.length || queued ? 'Waiting' : 'Ready';
}
