import { ChevronLeft } from 'lucide-react';
import { Fragment, useState } from 'react';
import { Button } from '@/components/ui/button';
import type { Failure, Task } from '../api';
import { began } from '../chat/team';
import { Face, GathrFace, YouFace } from '../components/Face';
import { FailureNotice } from '../components/Notice';
import { asker, briefRows, edited, gathrs, idle, noteLines, reported, reviewedAt } from './model';
import { TaskChecks } from './TaskChecks';
import { Line, TaskState } from './TaskList';

type Act = 'cancel' | 'restart';

/** What acts on a task in each state (0010): a done task has nothing, an open one only Cancel, an ended one Restart. */
export const OFFERS: Record<Task['state'], Act[]> = {
  done: [],
  waiting: ['cancel'],
  ready: ['cancel'],
  working: ['cancel'],
  held: ['cancel'],
  blocked: ['cancel', 'restart'],
  failed: ['cancel', 'restart'],
  stopped: ['cancel', 'restart'],
  cancelled: ['restart'],
};

/** Back to the list, and the task's number. */
export function Top({ number, onBack }: { number: number; onBack: () => void }) {
  return (
    <div className="tasks-top">
      <button type="button" className="tasks-back" onClick={onBack}>
        <ChevronLeft aria-hidden="true" />
        Tasks
      </button>
      <span>#{number}</span>
    </div>
  );
}

interface Props {
  task: Task;
  /** The chat's tasks: a waiting task names those it comes after. */
  tasks: Task[];
  /** The chat's project: Gathr's checks run there, so a task with a PR shows its run (0043 C6). */
  project?: string | null;
  name: (agent: string | null) => string;
  /** Settles once the service has done it; a refusal stays on this view with its reference. */
  onSettle: (task: Task, action: Act) => Promise<unknown>;
  onReference: (ref: string) => void;
  onBack: () => void;
}

/**
 * An agent's task (the tasks mock-up): its owner and state on the line under its what, who gave it and why, its
 * brief, what its owner reported and what it waits on (0047), its PR's checks run, and a foot only for what acts.
 */
export function TaskView({ task, tasks, project, name, onSettle, onReference, onBack }: Props) {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const settle = (action: Act) => {
    setBusy(true);
    setFailure(null);
    onSettle(task, action)
      .catch(setFailure)
      .finally(() => setBusy(false));
  };
  const offers = OFFERS[task.state];
  const report = reported(task);
  const review = reviewedAt(task);
  const notes = noteLines(task);
  const brief = [
    ...briefRows(
      task,
      task.after.map((n) => `#${n}`),
    ),
    ['Edited', edited(task)],
  ];
  // Report and Waiting on already say why a task is held, a report's note is often the reason word
  // for word, and a task stopped as left or with no report says so in Report. A reason they don't
  // carry stays: "blocked on #3" from a report with no note, "#18 merged".
  const stopped = task.report ? task.report.how === 'left' : !!task.asked;
  const told =
    task.state === 'held' || (task.state === 'blocked' && stopped) || task.reason === task.report?.note;
  const owner = task.owner && (
    <span className="task-owner">
      <Face agent={{ id: task.owner, name: name(task.owner) }} size={16} />
      {name(task.owner)}
    </span>
  );
  return (
    <section className="tasks" aria-label={`Task #${task.number}`}>
      <div className="tasks-scroll">
        <div className="tasks-page">
          {failure && <FailureNotice failure={failure} onReference={onReference} />}
          <Top number={task.number} onBack={onBack} />
          <h2>{task.what}</h2>
          <Line
            className="task-view-line"
            pieces={[
              owner,
              task.look_into && 'Look into',
              task.pr && `PR #${task.pr}`,
              task.state !== 'done' && <TaskState key="state" task={task} tasks={tasks} />,
              task.stale_us && idle(task.stale_us),
            ]}
          />
          <dl className="task-fields">
            <dt>From</dt>
            <dd className="task-from">
              {gathrs(task) ? (
                <GathrFace size={20} />
              ) : task.giver ? (
                <Face agent={{ id: task.giver, name: name(task.giver) }} size={20} />
              ) : (
                <YouFace size={20} />
              )}
              <span>{asker(task, name)}</span>
              <small>{began(task.created_us)}</small>
            </dd>
            {task.why && (
              <>
                <dt>Why</dt>
                <dd className="task-why">{task.why}</dd>
              </>
            )}
            {task.reason && !told && (
              <>
                <dt>Reason</dt>
                <dd>{task.reason}</dd>
              </>
            )}
            {brief.map(
              ([label, value]) =>
                value && (
                  <Fragment key={label}>
                    <dt>{label}</dt>
                    <dd>{value}</dd>
                  </Fragment>
                ),
            )}
            {report && (
              <>
                <dt>Report</dt>
                <dd>{report}</dd>
              </>
            )}
            {review && (
              <>
                <dt>Reviewed at</dt>
                <dd>{review}</dd>
              </>
            )}
            {notes.length > 0 && (
              <>
                <dt>Notes</dt>
                {notes.map((line, i) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: two notes may read the same
                  <dd key={i}>{line}</dd>
                ))}
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
      {offers.length > 0 && (
        <footer className="tasks-footer task-foot">
          {offers.includes('cancel') && (
            <Button size="sm" variant="outline" disabled={busy} onClick={() => settle('cancel')}>
              Cancel
            </Button>
          )}
          {offers.includes('restart') && (
            <Button
              size="sm"
              variant={offers.length > 1 ? 'default' : 'outline'}
              disabled={busy}
              onClick={() => settle('restart')}
            >
              Restart
            </Button>
          )}
        </footer>
      )}
    </section>
  );
}
