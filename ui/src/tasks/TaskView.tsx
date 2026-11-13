import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure, type Task } from '../api';
import { FailureNotice } from '../components/Notice';
import { listen } from '../live';
import { idle, reported } from './model';
import { TaskChecks } from './TaskChecks';

/** Only a settled task starts again (0010); an open one the person can only cancel. */
const restartable = new Set(['failed', 'stopped', 'blocked', 'cancelled']);

interface Props {
  task: Task;
  chatTitle?: string;
  /** The chat's project: Gathr's checks run there, so a task with a PR shows its run (0043 C6). */
  project?: string | null;
  name: (agent: string | null) => string;
  /** Settles once the service has done it; a refusal stays on this view with its reference. */
  onSettle: (task: Task, action: 'cancel' | 'restart') => Promise<unknown>;
  onReference: (ref: string) => void;
  /** Back to the chat's questions, for a task opened from Summary or a hand-off line. */
  onBack?: () => void;
}

/**
 * An agent's task (archive-stale/v1, the task-pass and task-fail mock-ups): who has it, its state, what
 * its owner reported and what it waits on (0047), how long it's been idle once stale, its PR's checks
 * run, and Cancel and Restart.
 */
export function TaskView({ task, chatTitle, project, name, onSettle, onReference, onBack }: Props) {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const settle = (action: 'cancel' | 'restart') => {
    setBusy(true);
    setFailure(null);
    onSettle(task, action)
      .catch(setFailure)
      .finally(() => setBusy(false));
  };
  const meta = [
    name(task.giver),
    chatTitle,
    task.pr ? `PR #${task.pr}` : undefined,
    task.stale_us ? idle(task.stale_us) : undefined,
  ].filter(Boolean);
  const report = reported(task);
  // Report and Waiting on already say why a task is held, or stopped by its owner's word or silence. A
  // reason they don't carry stays: "blocked on #3", from a report with a task and no note.
  const said = task.report ? task.report.how === 'left' || !!task.report.note : !!task.asked;
  const told = task.state === 'held' || (task.state === 'blocked' && said);
  return (
    <section className="tasks" aria-label={`Task #${task.number}`}>
      <div className="tasks-scroll">
        <div className="tasks-page">
          {failure && <FailureNotice failure={failure} onReference={onReference} />}
          <p className="task-number">Task #{task.number}</p>
          <h2>{task.what}</h2>
          <p className="tasks-meta">{meta.join(' · ')}</p>
          <dl className="task-fields">
            <dt>Assigned to</dt>
            <dd>{name(task.owner)}</dd>
            <dt>State</dt>
            <dd>{task.state}</dd>
            {task.reason && !told && (
              <>
                <dt>Reason</dt>
                <dd>{task.reason}</dd>
              </>
            )}
            {report && (
              <>
                <dt>Report</dt>
                <dd>{report}</dd>
              </>
            )}
            {!!task.waiting_on?.length && (
              <>
                <dt>Waiting on</dt>
                <dd>{task.waiting_on.join(', ')}</dd>
              </>
            )}
          </dl>
          {project && task.pr_head && (
            <TaskChecks project={project} head={task.pr_head} onReference={onReference} />
          )}
        </div>
      </div>
      <footer className="tasks-footer">
        <div className="tasks-pager">
          {onBack ? (
            <Button size="sm" variant="ghost" onClick={onBack}>
              Back
            </Button>
          ) : (
            <span />
          )}
          <span role="status">{busy ? 'Saving' : ''}</span>
          <div>
            <Button size="sm" variant="ghost" disabled={busy} onClick={() => settle('cancel')}>
              Cancel
            </Button>
            <Button
              size="sm"
              disabled={busy || !restartable.has(task.state)}
              onClick={() => settle('restart')}
            >
              Restart
            </Button>
          </div>
        </div>
      </footer>
    </section>
  );
}

/** The chat's task by number, live: it moves with each change to the chat's tasks. A load that works,
 * or a live list, clears an earlier failure; another task starts afresh. */
export function useTask(chat: string, number: number) {
  const [tasks, setTasks] = useState<Task[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: another task starts afresh, without the last one's failure
  useEffect(() => {
    let current = true;
    setTasks(null);
    setFailure(null);
    const got = (list: Task[]) => {
      if (!current) return;
      setTasks(list);
      setFailure(null);
    };
    const load = () =>
      api.tasks(chat).then(
        (t) => got(t.tasks),
        (f: Failure) => current && setFailure(f),
      );
    load();
    const off = listen((e) => {
      if (e.type === 'tasks' && e.chat === chat) got(e.tasks);
      if (e.type === 'lagged') load();
    });
    return () => {
      current = false;
      off();
    };
  }, [chat, number]);
  return { tasks, task: tasks?.find((t) => t.number === number) ?? null, failure };
}
