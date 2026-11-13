import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Failure } from '../api';
import { FailureNotice } from '../components/Notice';
import { readyVersion, useAppUpdate } from '../update';

/** How Gathr's update stands, in a few words. */
const standing: Record<string, string> = {
  off: 'This build does not update',
  idle: 'Up to date',
  checking: 'Checking for updates',
  updating: 'Updating Gathr',
};

/**
 * Settings > General: the version that runs, Update when a release is ready, a check now, and the
 * last failure with its reference (0032). Gathr never updates by itself.
 */
export function AppVersion({ onReference }: { onReference: (ref: string) => void }) {
  const [refused, setRefused] = useState<Failure | null>(null);
  const { app, asking, update, check } = useAppUpdate(setRefused);
  if (!app?.version) return null;
  const ready = readyVersion(app);
  const failure = refused ?? (app.failed && new Failure('Update failed', app.failed.why, app.failed.ref));
  const act = (action: () => void) => () => {
    setRefused(null);
    action();
  };
  return (
    <div className="settingrow">
      <div>
        <h4>Version {app.version}</h4>
        <p className="muted" role="status">
          {ready
            ? `${ready} is ready`
            : app.failed && app.state === 'idle'
              ? 'The last check failed'
              : standing[app.state]}
        </p>
      </div>
      {ready ? (
        <Button type="button" onClick={act(update)} disabled={asking}>
          Update to {ready}
        </Button>
      ) : (
        app.state !== 'off' && (
          <Button type="button" variant="outline" onClick={act(check)} disabled={app.state !== 'idle'}>
            Check for updates
          </Button>
        )
      )}
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
    </div>
  );
}
