import type { ThreadMessageLike } from '@assistant-ui/react';
import { memo } from 'react';
import { ContextDisplay } from '@/components/assistant-ui/elements/context-display';
import { type TodoItem, TodoList } from '@/components/assistant-ui/elements/todo-list';
import { Button } from '@/components/ui/button';
import type { Member, Task } from '../api';
import { ChatMenu } from './ChatMenu';
import type { Work } from './fold';
import type { Glance } from './glance';
import { Initial } from './Initial';
import type { Custom } from './messages';
import { type Actions, Blocks } from './Thread';
import { WorkRow } from './TurnParts';
import { open, roleName } from './team';
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

/** What `blocksOf` made on earlier draws, for one agent. */
export interface Kept {
  blocks: WeakMap<ThreadMessageLike, ThreadMessageLike>;
  earlier: ThreadMessageLike[];
}

export const noneKept = (): Kept => ({ blocks: new WeakMap(), earlier: [] });

/**
 * An agent's part of the timeline: its running turn's row, and its earlier blocks, newest first, the latest
 * ten. A block of its own is headed by its name, as none other sits between them here; of a talk between
 * agents, only what it wrote. A block made on an earlier draw is the same object, and so is the list while
 * none of its blocks changed (`kept`): another agent's token leaves the panel's inputs as they were.
 */
export function blocksOf(items: ThreadMessageLike[], agent: string, kept: Kept) {
  let now: Work | undefined;
  const all: ThreadMessageLike[] = [];
  const keep = (item: ThreadMessageLike, make: () => ThreadMessageLike) => {
    const value = kept.blocks.get(item) ?? make();
    kept.blocks.set(item, value);
    all.push(value);
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
  const earlier = all.reverse().slice(0, EARLIER);
  const same = earlier.length === kept.earlier.length && earlier.every((b, at) => b === kept.earlier[at]);
  if (!same) kept.earlier = earlier;
  return { now, earlier: kept.earlier };
}

interface Props {
  glance: Glance;
  /** The chat's tasks and members, as the chat's state holds them. */
  tasks: Task[];
  members: Member[];
  /** Its running turn's row and its earlier blocks, from blocksOf. */
  now?: Work;
  earlier: ThreadMessageLike[];
  actions: Actions;
  /** Picks the agent as the composer's recipient and focuses the composer; none when it can't be sent to. */
  onMessage?: (agent: string) => void;
  /** Compacts its conversation in this chat; none where it can't (an archived chat). */
  onCompact?: (agent: string) => void;
  /** A compaction asked for here: queued behind its work, or running. */
  compaction?: 'queued' | 'running';
}

/**
 * One agent beside the chat, as a workspace tab (DESIGN.md "Agent panel", 0038): who it is and how it
 * stands, the task it is on, its running turn's row opened, its earlier blocks in this chat and its
 * tasks. The row and the blocks are the timeline's own components. Drawn again only when something of
 * its own changed: every prop keeps its identity through another agent's streamed token.
 */
export const AgentPanel = memo(function AgentPanel(props: Props) {
  const { glance, tasks, members, now, earlier, actions, onMessage, onCompact, compaction } = props;
  const { member, standing } = glance;
  const mine = tasks
    .filter((t) => t.owner === member.agent && t.state !== 'cancelled')
    .sort((a, b) => a.number - b.number);
  // The task its running turn works on, else its first open one.
  const task = mine.find((t) => t.number === glance.task) ?? mine.find(open);
  const giver = (t: Task) =>
    t.giver === null ? 'you' : (members.find((m) => m.agent === t.giver)?.name ?? 'an agent');
  const { used, window } = member.context ?? {};
  const nothing = !task && !now && standing !== 'working' && !earlier.length && !mine.length;
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
            <Button type="button" variant="outline" onClick={() => onMessage(member.agent)}>
              Message {member.name}
            </Button>
          )}
          {onCompact && (
            <ChatMenu
              label={`More for ${member.name}`}
              compact={{ asked: !!compaction, onCompact: () => onCompact(member.agent) }}
            />
          )}
        </span>
      </header>
      {compaction && (
        <p className="agent-quiet">
          {compaction === 'running'
            ? 'Compacting'
            : 'Compaction queued · waits for its turn and background commands'}
        </p>
      )}
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
});
