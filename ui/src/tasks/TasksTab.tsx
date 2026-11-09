import { useLayoutEffect, useRef, useState } from 'react';
import type { Failure } from '../api';
import { Notice } from '../components/Notice';
import { type Draft, type PersonTask, ready } from './model';
import { TaskAnswer } from './TaskAnswer';
import './tasks.css';

export interface Answer {
  task: PersonTask;
  draft: Draft;
}

interface Props {
  /** The chat's open questions for the person, oldest first. */
  tasks: PersonTask[];
  selected: string | null;
  onSelect: (id: string) => void;
  drafts: Record<string, Draft>;
  onDraft: (id: string, draft: Draft) => void;
  onOpenChat: (chat: string) => void;
  /** Sends the ready answers together; answers the ones that failed, which stay open. */
  onSend: (answers: Answer[]) => Promise<Failure[]>;
  onReference?: (ref: string) => void;
}

/**
 * The chat's questions for the person, one at a time (DESIGN.md "Human tasks"). Back and Next keep
 * drafts; Send on the last one sends every ready answer at once, and the rest stay open.
 */
export function TasksTab({
  tasks,
  selected,
  onSelect,
  drafts,
  onDraft,
  onOpenChat,
  onSend,
  onReference,
}: Props) {
  const scroll = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const [sending, setSending] = useState(false);
  const [failures, setFailures] = useState<Failure[]>([]);
  const at = Math.max(
    0,
    tasks.findIndex((t) => t.id === selected),
  );
  const task = tasks[at];
  const done = tasks.filter((t) => ready(t, drafts[t.id])).length;

  // biome-ignore lint/correctness/useExhaustiveDependencies: a new question starts at its top
  useLayoutEffect(() => {
    if (scroll.current) scroll.current.scrollTop = 0;
  }, [task?.id]);

  const go = (to: number) => {
    onSelect(tasks[to].id);
    requestAnimationFrame(() => heading.current?.focus({ preventScroll: true }));
  };
  const send = async () => {
    setSending(true);
    setFailures(
      await onSend(
        tasks.filter((t) => ready(t, drafts[t.id])).map((t) => ({ task: t, draft: drafts[t.id] })),
      ),
    );
    setSending(false);
  };

  if (!task) return <p className="tasks-empty">No open tasks</p>;
  const last = at === tasks.length - 1;
  return (
    <section className="tasks" aria-label="Your open tasks">
      <div className="tasks-scroll" ref={scroll}>
        <div className="tasks-page">
          {failures.map((f) => (
            <Notice
              key={f.ref ?? f.message}
              danger
              title={f.message}
              reference={f.ref && onReference ? { ref: f.ref, open: onReference } : undefined}
            />
          ))}
          <h2 ref={heading} tabIndex={-1}>
            {task.title}
          </h2>
          <p className="tasks-meta">
            <span>{task.agent}</span>
            <span aria-hidden="true">·</span>
            <a
              href={`#${task.chat}`}
              onClick={(e) => {
                e.preventDefault();
                onOpenChat(task.chat);
              }}
            >
              {task.chatTitle}
            </a>
            {task.file && (
              <>
                <span aria-hidden="true">·</span>
                <span>{task.file}</span>
              </>
            )}
          </p>
          <TaskAnswer
            key={task.id}
            task={task}
            draft={drafts[task.id] ?? { values: [], notes: '' }}
            onDraft={(d) => onDraft(task.id, d)}
          />
        </div>
      </div>
      <footer className="tasks-footer">
        <div className="tasks-pager">
          <span>
            {at + 1} of {tasks.length}
          </span>
          <span role="status">{done} Done</span>
          <div>
            <button type="button" disabled={at === 0} onClick={() => go(at - 1)}>
              Back
            </button>
            <button
              type="button"
              className="primary"
              disabled={last && (sending || done === 0)}
              onClick={() => (last ? send() : go(at + 1))}
            >
              {last ? 'Send' : 'Next'}
            </button>
          </div>
        </div>
      </footer>
    </section>
  );
}
