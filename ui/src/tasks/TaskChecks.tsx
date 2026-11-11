import { Check, CircleAlert, LoaderCircle } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { api, type CheckRun, type Failure } from '../api';
import { FailureNotice } from '../components/Notice';
import { listen } from '../live';
import { seconds } from '../settings/timeline';

const said: Record<CheckRun['state'], string> = {
  running: 'Checks running',
  passed: 'Checks passed',
  failed: 'Checks failed',
};
const icons = { running: LoaderCircle, passed: Check, failed: CircleAlert };

interface Props {
  project: string;
  /** The task's PR head as Gathr last saw it: the commit whose run shows. */
  head: string;
  onReference: (ref: string) => void;
}

/**
 * Gathr's checks run on a task's PR at its head (0036 C6, the task-pass and task-fail mock-ups): passed,
 * failed with its reference, or running, then each command with its exit, time and log, the failing
 * one's open. Live: a run of the project at this commit reloads it.
 */
export function TaskChecks({ project, head, onReference }: Props) {
  const [runs, setRuns] = useState<CheckRun[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const title = useId();
  // Each project and head has its own answer: a change starts afresh, and a late answer for the
  // head before is dropped, never shown over the new one's (Vale, #121).
  useEffect(() => {
    let current = true;
    setRuns(null);
    setFailure(null);
    const load = () =>
      api.checkRuns(project, head).then(
        (found) => {
          if (!current) return;
          setRuns(found);
          setFailure(null);
        },
        (f: Failure) => current && setFailure(f),
      );
    load();
    const off = listen((e) => {
      if (e.type === 'lagged' || (e.type === 'checks' && e.project === project && e.head === head)) load();
    });
    return () => {
      current = false;
      off();
    };
  }, [project, head]);

  if (failure) return <FailureNotice failure={failure} onReference={onReference} />;
  if (runs === null) return null;
  // Newest first: a commit run again after a stop shows its latest run.
  const run = runs.find((r) => r.head === head);
  if (!run) return <p className="muted task-checks">No checks run at this commit yet</p>;
  const Icon = icons[run.state];
  const total = run.steps.reduce((sum, s) => sum + s.duration_ms, 0);
  const ended = run.ended ? new Date(run.ended) : null;
  return (
    <section className="task-checks" data-state={run.state} aria-labelledby={title}>
      <h3 id={title}>
        <Icon aria-hidden="true" className={run.state === 'running' ? 'animate-spin' : undefined} />
        {said[run.state]}
      </h3>
      <p className="muted">
        {run.steps.length} {run.steps.length === 1 ? 'command' : 'commands'} · {seconds(total)}
      </p>
      <dl className="task-fields">
        <dt>Commit</dt>
        <dd className="mono">{run.head.slice(0, 7)}</dd>
        {ended && (
          <>
            <dt>Finished</dt>
            <dd>{ended.toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })}</dd>
          </>
        )}
      </dl>
      {run.state === 'failed' && (
        <FailureNotice
          failure={{ error: run.why ?? 'A check failed', why: null, ref: run.ref }}
          onReference={onReference}
        />
      )}
      {run.steps.map((s) => {
        const failed = s.exit !== 0;
        return (
          <details key={s.command} className="task-step" open={failed}>
            <summary>
              {failed ? <CircleAlert aria-hidden="true" /> : <Check aria-hidden="true" />}
              <code>{s.command}</code>
              <small>
                Exit {s.exit ?? '–'} · {seconds(s.duration_ms)}
              </small>
            </summary>
            {s.log ? <pre>{s.log}</pre> : <p className="muted">No output</p>}
          </details>
        );
      })}
    </section>
  );
}
