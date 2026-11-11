import { Fragment, useLayoutEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import type { PersonTask, StaleTask, Task } from '../api';
import { FailureNotice, Notice } from '../components/Notice';
import { type Draft, ready } from './model';
import { TaskAnswer } from './TaskAnswer';
import { TaskView, useTask } from './TaskView';
import './tasks.css';

/** A question Send didn't settle: it stays open, with why and its reference. */
export interface Refusal {
  task: number;
  why: string;
  ref: string | null;
}

interface Props {
  /** The chat's open questions for the person, oldest first. */
  tasks: PersonTask[];
  selected: number | null;
  onSelect: (task: number) => void;
  drafts: Record<number, Draft>;
  onDraft: (task: number, draft: Draft) => void;
  /** An agent's name by its id. */
  name: (agent: string | null) => string;
  onOpenFile: (task: PersonTask, file: number) => void;
  /** Sends the ready answers together; answers the ones refused, which stay open. */
  onSend: (tasks: PersonTask[]) => Promise<Refusal[]>;
  onReference: (ref: string) => void;
  /** The chat's stale agent tasks (F11): each opens as its own view, outside the questions' pager. */
  stale?: StaleTask[];
  onSettle?: (task: Task, action: 'cancel' | 'restart') => Promise<unknown>;
  /** An agent's task opened from Summary or a hand-off line, shown until Back. */
  opened?: { chat: string; task: number; project?: string | null; onBack: () => void };
}

/**
 * The chat's questions for the person, one at a time (DESIGN.md "Human tasks"). Back and Next keep
 * drafts; Send on the last one sends every ready answer at once, and the rest stay open.
 */
export function TasksTab(props: Props) {
  const { tasks, selected, onSelect, drafts, onDraft, name, onOpenFile, onSend, onReference } = props;
  const scroll = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const [sending, setSending] = useState(false);
  const [refused, setRefused] = useState<Refusal[]>([]);
  const at = Math.max(
    0,
    tasks.findIndex((t) => t.number === selected),
  );
  const task = tasks[at];
  const staged = tasks.filter((t) => ready(t, drafts[t.number])).length;

  const moved = useRef(false);
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new question starts at its top
  useLayoutEffect(() => {
    if (scroll.current) scroll.current.scrollTop = 0;
    // Before paint, never a frame later: late focus took keystrokes from the field being typed in.
    if (moved.current) heading.current?.focus({ preventScroll: true });
    moved.current = false;
  }, [task?.number]);

  const go = (to: number) => {
    moved.current = true;
    onSelect(tasks[to].number);
  };
  const send = async () => {
    setSending(true);
    const refusals = await onSend(tasks.filter((t) => ready(t, drafts[t.number])));
    setRefused(refusals);
    setSending(false);
    // The first refused question, with why, so it isn't missed.
    if (refusals.length) onSelect(refusals[0].task);
  };

  if (props.opened && props.onSettle)
    return <OpenedTask {...props.opened} name={name} onSettle={props.onSettle} onReference={onReference} />;
  const agentTask = props.stale?.find((t) => t.number === selected) ?? (task ? undefined : props.stale?.[0]);
  if (agentTask && props.onSettle)
    return (
      <TaskView
        key={agentTask.number}
        task={agentTask}
        chatTitle={agentTask.chat_title}
        name={name}
        onSettle={props.onSettle}
        onReference={onReference}
      />
    );
  if (!task) return <p className="tasks-empty">No open tasks</p>;
  const last = at === tasks.length - 1;
  const why = refused.find((r) => r.task === task.number);
  return (
    <section className="tasks" aria-label="Your open tasks">
      <div className="tasks-scroll" ref={scroll}>
        <div className="tasks-page">
          {why && (
            <Notice
              danger
              title={`Answer failed: ${why.why}`}
              reference={why.ref ? { ref: why.ref, open: onReference } : undefined}
            />
          )}
          <h2 ref={heading} tabIndex={-1}>
            {task.what}
          </h2>
          <p className="tasks-meta">
            <span>
              {name(task.giver)}
              {task.for_agent && ` for ${name(task.for_agent)}`}
            </span>
            <span aria-hidden="true">·</span>
            <span>{task.chat_title}</span>
            {task.files.map((f, i) => (
              <Fragment key={f.name}>
                <span aria-hidden="true">·</span>
                <a
                  href={`/api/tasks/${task.number}/files/${i}`}
                  onClick={(e) => {
                    e.preventDefault();
                    onOpenFile(task, i);
                  }}
                >
                  {f.name}
                </a>
              </Fragment>
            ))}
          </p>
          {task.why && <p className="tasks-why">{task.why}</p>}
          <TaskAnswer
            key={task.number}
            task={task}
            draft={drafts[task.number] ?? { values: [], notes: '' }}
            onDraft={(d) => onDraft(task.number, d)}
          />
        </div>
      </div>
      <footer className="tasks-footer">
        <div className="tasks-pager">
          <span>
            {at + 1} of {tasks.length}
          </span>
          <span role="status">{staged} ready</span>
          <div>
            <Button size="sm" variant="ghost" disabled={at === 0} onClick={() => go(at - 1)}>
              Back
            </Button>
            <Button
              size="sm"
              disabled={last && (sending || staged === 0)}
              onClick={() => (last ? send() : go(at + 1))}
            >
              {last ? 'Send' : 'Next'}
            </Button>
          </div>
        </div>
      </footer>
    </section>
  );
}

type Opened = NonNullable<Props['opened']> &
  Pick<Props, 'name' | 'onReference'> & {
    onSettle: NonNullable<Props['onSettle']>;
  };

/** An agent's task the person opened, kept live as the chat's tasks change. */
function OpenedTask({ chat, task: number, project, onBack, name, onSettle, onReference }: Opened) {
  const { task, failure } = useTask(chat, number);
  if (failure) return <FailureNotice failure={failure} onReference={onReference} />;
  if (!task) return null;
  return (
    <TaskView
      key={task.number}
      task={task}
      project={project}
      name={name}
      onSettle={onSettle}
      onReference={onReference}
      onBack={onBack}
    />
  );
}
