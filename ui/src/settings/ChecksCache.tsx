import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure } from '../api';
import { FailureNotice } from '../components/Notice';
import { listen } from '../live';

const units = ['B', 'KB', 'MB', 'GB', 'TB'];

/** 2.4 GB: decimal units, as the system's file managers show sizes. */
export function size(bytes: number): string {
  let n = bytes;
  let unit = 0;
  while (n >= 1000 && unit < units.length - 1) {
    n /= 1000;
    unit += 1;
  }
  return `${n.toLocaleString([], { maximumFractionDigits: 1 })} ${units[unit]}`;
}

/**
 * A project's checks cache (0043 C3, the mock-up's row): its size and Clear, which acts at once, apart
 * from Save. Clearing waits for a run that's using the cache, so it says so meanwhile.
 */
export function ChecksCache({
  project,
  onReference,
}: {
  project: string;
  onReference: (ref: string) => void;
}) {
  const [bytes, setBytes] = useState<number | null>(null);
  const [clearing, setClearing] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const load = useCallback(() => api.caches(project).then(setBytes, setFailure), [project]);
  useEffect(() => {
    load();
    // A run that ends has filled the cache, or a clear elsewhere emptied it.
    return listen((e) => {
      if (e.type === 'lagged' || (e.type === 'checks' && e.project === project && e.state !== 'running'))
        load();
    });
  }, [load, project]);

  const clear = () => {
    setClearing(true);
    setFailure(null);
    api
      .clearCaches(project)
      .then(load, setFailure)
      .finally(() => setClearing(false));
  };

  return (
    <div className="agent-field project-gate">
      <h4>Checks cache</h4>
      <div className="project-cache">
        <p className="muted">
          {bytes != null && <strong>{size(bytes)}</strong>} · Unused caches are removed after 14 days.
        </p>
        <span className="muted" aria-live="polite">
          {clearing ? 'Clearing after the running checks' : ''}
        </span>
        <Button type="button" variant="outline" disabled={clearing || !bytes} onClick={clear}>
          Clear
        </Button>
      </div>
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
    </div>
  );
}
