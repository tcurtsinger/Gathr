import { useState } from 'react';
import { Button } from '@/components/ui/button';
import type { Failure, StaleTask } from '../api';
import { FailureNotice } from '../components/Notice';
import { idle } from './model';

/** Only a settled task starts again (0010); an open one the person can only cancel. */
const restartable = new Set(['failed', 'stopped', 'blocked', 'cancelled']);

interface Props {
  task: StaleTask;
  name: (agent: string | null) => string;
  /** Settles once the service has done it; a refusal stays on this view with its reference. */
  onSettle: (task: StaleTask, action: 'cancel' | 'restart') => Promise<unknown>;
  onReference: (ref: string) => void;
}

/** An agent's stale task (archive-stale/v1): who has it, its state and reason, Cancel and Restart. */
export function StaleTaskView({ task, name, onSettle, onReference }: Props) {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const settle = (action: 'cancel' | 'restart') => {
    setBusy(true);
    setFailure(null);
    onSettle(task, action)
      .catch(setFailure)
      .finally(() => setBusy(false));
  };
  return (
    <section className="tasks" aria-label="Stale task">
      <div className="tasks-scroll">
        <div className="tasks-page">
          {failure && <FailureNotice failure={failure} onReference={onReference} />}
          <h2>{task.what}</h2>
          <p className="tasks-meta">
            <span>{name(task.giver)}</span>
            <span aria-hidden="true">·</span>
            <span>{task.chat_title}</span>
            <span aria-hidden="true">·</span>
            <span>{task.stale_us && idle(task.stale_us)}</span>
          </p>
          <dl className="task-fields">
            <dt>Assigned to</dt>
            <dd>{name(task.owner)}</dd>
            <dt>State</dt>
            <dd>{task.state}</dd>
            {task.reason && (
              <>
                <dt>Reason</dt>
                <dd>{task.reason}</dd>
              </>
            )}
          </dl>
        </div>
      </div>
      <footer className="tasks-footer">
        <div className="tasks-pager">
          <span />
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
