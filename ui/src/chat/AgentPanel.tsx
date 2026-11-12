import type { ThreadMessageLike } from '@assistant-ui/react';
import { useMemo, useRef } from 'react';
import { ContextDisplay } from '@/components/assistant-ui/elements/context-display';
import { type TodoItem, TodoList } from '@/components/assistant-ui/elements/todo-list';
import { Button } from '@/components/ui/button';
import type { Task } from '../api';
import type { Work } from './fold';
import type { Glance } from './glance';
import { Initial } from './Initial';
import type { Custom } from './messages';
import type { ChatState } from './state';
import { type Actions, Blocks } from './Thread';
import { WorkRow } from './TurnParts';
import { roleName } from './team';
import { open } from './waits';
import './agent.css';

/** The agent's earlier blocks shown; older ones stay in the chat. */
const EARLIER = 10;
/** Tasks its list shows before Show more; open ones always show. */
const FOLD = 5;

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

type Kept = WeakMap<ThreadMessageLike, ThreadMessageLike>;

/**
 * An agent's part of the timeline: its running turn's row, and its earlier blocks, newest first. A block
 * of its own is headed by its name, as none other sits between them here; of a talk between agents, only
 * what it wrote. A block made on an earlier draw is the same object (`kept`), so it isn't converted again.
 */
export function blocksOf(items: ThreadMessageLike[], agent: string, kept: Kept) {
  let now: Work | undefined;
  const earlier: ThreadMessageLike[] = [];
  const keep = (item: ThreadMessageLike, make: () => ThreadMessageLike) => {
    const value = kept.get(item) ?? make();
    kept.set(item, value);
    earlier.push(value);
  };
  for (const item of items) {
    const custom = (item.metadata?.custom ?? {}) as Custom;
    if (custom.exchange) {
      const own = custom.exchange.flatMap((m, at) => (m.by === agent ? at : []));
      if (!own.length || typeof item.content === 'string') continue;
      const { content } = item;
      keep(item, () => ({
        ...item,
        content: own.map((at) => content[at]),
        metadata: { custom: { exchange: own.map((at) => custom.exchange?.[at]) } },
      }));
    } else if (custom.by === agent && item.role === 'assistant') {
      if (item.status?.type === 'running') now = custom.work ?? now;
      else keep(item, () => ({ ...item, metadata: { custom: { ...custom, head: true } } }));
    }
  }
  return { now, earlier: earlier.reverse().slice(0, EARLIER) };
}

interface Props {
  glance: Glance;
  state: ChatState;
  /** The chat's timeline, from toThread. */
  items: ThreadMessageLike[];
  actions: Actions;
  /** Picks the agent as the composer's recipient and focuses the composer; none when it can't be sent to. */
  onMessage?: () => void;
}

/**
 * One agent beside the chat, as a workspace tab (DESIGN.md "Agent panel", 0038): who it is and how it
 * stands, the task it is on, its running turn's row opened, its earlier blocks in this chat and its
 * tasks. The row and the blocks are the timeline's own components.
 */
export function AgentPanel({ glance, state, items, actions, onMessage }: Props) {
  const { member, standing } = glance;
  const kept = useRef<Kept>(new WeakMap());
  const { now, earlier } = useMemo(() => blocksOf(items, member.agent, kept.current), [items, member.agent]);
  const running = Object.values(state.turns).filter(
    (t) => t.agent === member.agent && t.status === 'running',
  );
  const mine = state.tasks
    .filter((t) => t.owner === member.agent && t.state !== 'cancelled')
    .sort((a, b) => a.number - b.number);
  // The task its running turn works on, else its first open one.
  const task = mine.find((t) => running.some((turn) => turn.task === t.number)) ?? mine.find(open);
  const giver = (t: Task) =>
    t.giver === null ? 'you' : (state.members.find((m) => m.agent === t.giver)?.name ?? 'an agent');
  const { used, window } = member.context ?? {};
  const nothing = !task && !running.length && !earlier.length && !mine.length;
  return (
    <div className="agent-panel">
      <header className="agent-who">
        <Initial name={member.name} size="l" busy={standing === 'working'} />
        <div>
          <h2>{member.name}</h2>
          <p>
            {roleName[member.role]} ·{' '}
            {standing === 'working' ? (
              <>
                <b>Working</b>
                {glance.began && ` since ${glance.began}`}
              </>
            ) : standing === 'needs' ? (
              <b>{glance.doing}</b>
            ) : (
              (glance.detail ?? glance.doing)
            )}
          </p>
        </div>
        <span className="agent-actions">
          {/* How full its context is (compaction plan, K2): none until the engine has said. */}
          {used != null && window ? (
            <ContextDisplay.Bar modelContextWindow={window} usage={{ totalTokens: used }} side="bottom" />
          ) : null}
          {onMessage && (
            <Button type="button" variant="outline" onClick={onMessage}>
              Message {member.name}
            </Button>
          )}
        </span>
      </header>
      {task && (
        <section aria-label="Task">
          <h3>Task</h3>
          <p className="agent-task">
            {task.what}
            <small>
              From {giver(task)} ·{' '}
              <button type="button" className="link" onClick={() => actions.openTask(task.number)}>
                Open task
              </button>
            </small>
          </p>
        </section>
      )}
      {/* A turn that has done nothing yet has no row: it says Working, unless it waits on the person. */}
      {(now || standing === 'working') && (
        <section aria-label="Now">
          <h3>Now</h3>
          {now ? (
            <WorkRow work={now} onOpenTask={actions.openTask} startOpen />
          ) : (
            <p className="agent-quiet">Working</p>
          )}
        </section>
      )}
      {earlier.length > 0 && (
        <section aria-label="Earlier in this chat">
          <h3>Earlier in this chat</h3>
          <Blocks messages={earlier} actions={actions} />
        </section>
      )}
      {mine.length > 0 && (
        <section aria-label="Tasks in this chat">
          <h3>Tasks in this chat</h3>
          <TodoList
            className="agent-todos"
            // Its open work first: an agent's earlier rounds can leave dozens done.
            items={[...mine.filter(open), ...mine.filter((t) => !open(t))].map(todo)}
            maxVisible={FOLD}
            onItemSelect={(item) => actions.openTask(Number(item.id))}
          />
        </section>
      )}
      {nothing && <p className="agent-quiet">Nothing in this chat yet</p>}
    </div>
  );
}
