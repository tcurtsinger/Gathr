import { Popover } from 'radix-ui';
import { useState } from 'react';
import { ContextDisplay } from '@/components/assistant-ui/elements/context-display';
import { type SubagentItem, SubagentList } from '@/components/assistant-ui/elements/subagent-list';
import { type TodoItem, TodoList } from '@/components/assistant-ui/elements/todo-list';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import type { Member, PauseScope, Pauses, Task } from '../api';
import { ChatMenu } from './ChatMenu';
import { agentPause, type PauseAction, type Run } from './pauses';
import type { ChatState } from './state';
import { clock, list, roleName } from './team';
import { initials } from './Working';
import { began, open, waitingOn } from './waits';
import './summary.css';

/** Tasks an agent's list shows before Show more; open ones always show. */
const FOLD = 5;

interface Row extends SubagentItem {
  member: Member;
  /** Done tasks over the agent's tasks in this chat, cancelled ones left out, as a percentage. */
  share: number;
  todos: TodoItem[];
}

/** A task as the agent's TodoList shows it: done, at work (working or held), failed, else pending. */
export function todo(t: Task): TodoItem {
  const id = String(t.number);
  if (t.state === 'done') return { id, text: t.what, status: 'done' };
  if (t.state === 'working' || t.state === 'held') return { id, text: t.what, status: 'active' };
  if (t.state === 'failed') return { id, text: t.what, status: 'failed', reason: t.reason ?? undefined };
  const outcome = t.state === 'blocked' ? 'Blocked' : t.state === 'stopped' ? 'Stopped' : undefined;
  const description = outcome && (t.reason ? `${outcome}: ${t.reason}` : outcome);
  return { id, text: t.what, status: 'pending', description };
}

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
  /** Opens a task in the workspace's Tasks tab. */
  onOpenTask?: (task: number) => void;
}

/**
 * Every agent of the chat at a glance: role, what it's doing or waiting for, and its tasks (DESIGN.md
 * "Summary"), with Pause or Resume for each agent alone. The prototype's Radix popover from the chat
 * header, so an agent's menu opens above it; Escape or a click outside closes it.
 */
export function Summary({ state, pauses, run, onPause, onOpenTask }: Props) {
  const members = [...state.members].sort((a, b) => rank(a) - rank(b));
  // A null owner is the person: a question the Supervisor put to them.
  const name = (agent: string | null) =>
    agent === null ? 'you' : (state.members.find((m) => m.agent === agent)?.name ?? 'an agent');
  const [expanded, setExpanded] = useState<string>();
  const rows = members.map((m): Row => {
    const mine = state.tasks
      .filter((t) => t.owner === m.agent && t.state !== 'cancelled')
      .sort((a, b) => a.number - b.number);
    // Its open work first: an agent's earlier rounds can leave dozens done.
    const todos = [...mine.filter(open), ...mine.filter((t) => !open(t))].map(todo);
    const done = todos.filter((t) => t.status === 'done').length;
    const now = doing(m, state, run, agentPause(pauses, m.agent), name);
    return {
      member: m,
      name: m.name,
      model: roleName[m.role],
      avatar: (
        <span className="summary-avatar" aria-hidden="true">
          {initials(m.name)}
        </span>
      ),
      detail: (
        <>
          {roleName[m.role]} · <span data-working={now === 'Working' || undefined}>{now}</span>
        </>
      ),
      count: todos.length ? `${done}/${todos.length}` : undefined,
      share: todos.length ? (100 * done) / todos.length : 0,
      todos,
    };
  });

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
          <SubagentList
            presentation="inline"
            agents={rows}
            completedCount={0}
            progress={rows.map((r) => r.share)}
            showSummary={false}
            summaryAgent={rows[0] as Row}
            expandedAgent={expanded}
            onExpandedAgentChange={setExpanded}
            renderAgentTasks={(row) => {
              const { todos } = row as Row;
              if (todos.length === 0) return <p className="summary-none">No assigned tasks</p>;
              return (
                <TodoList
                  className="summary-todos"
                  items={todos}
                  maxVisible={FOLD}
                  onItemSelect={onOpenTask && ((item) => onOpenTask(Number(item.id)))}
                />
              );
            }}
            renderAgentActions={(row) => {
              const { member } = row as Row;
              const own = agentPause(pauses, member.agent);
              const agent: PauseScope = { scope: 'agent', target: member.agent };
              const { used, window } = member.context ?? {};
              return (
                <span className="summary-actions">
                  {/* How full its context is (compaction plan, K2): none until the engine has said. */}
                  {used != null && window ? (
                    <ContextDisplay.Bar
                      modelContextWindow={window}
                      usage={{ totalTokens: used }}
                      side="bottom"
                    />
                  ) : null}
                  <ChatMenu
                    label={`More for ${member.name}`}
                    side="left"
                    pause={{
                      state: own.paused ? 'paused' : 'running',
                      onPause: () => onPause('pause', agent),
                      onResume: () => onPause('resume', agent),
                    }}
                  />
                </span>
              );
            }}
          />
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
