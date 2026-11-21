import { useMemo, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Failure } from '../api';
import { useCard } from '../components/Corner';
import { readyVersion, useAppUpdate } from '../update';
import { Row } from './Row';

/** How Gathr's update stands, in a few words. */
const standing: Record<string, string> = {
  off: 'This build does not update',
  idle: 'Up to date',
  checking: 'Checking for updates',
  updating: 'Updating Gathr',
};

/**
 * Settings > General: the version that runs, Update when a release is ready, a check now, and the
 * last failure (0032). Gathr never updates by itself.
 */
export function AppVersion() {
  const [refused, setRefused] = useState<Failure | null>(null);
  const { app, asking, update, check } = useAppUpdate(setRefused);
  const ready = readyVersion(app);
  // With nothing ready the row says "The last check failed"; with a release ready, only a card says the update failed.
  const { why, ref } = app?.failed ?? {};
  const last = useMemo(() => (ref ? new Failure('Update failed', why ?? null, ref) : null), [why, ref]);
  useCard(refused ?? (ready ? last : null));
  if (!app?.version) return null;
  const act = (action: () => void) => () => {
    setRefused(null);
    action();
  };
  return (
    <Row
      name={`Version ${app.version}`}
      line={
        <span role="status">
          {ready
            ? `${ready} is ready`
            : app.failed && app.state === 'idle'
              ? 'The last check failed'
              : standing[app.state]}
        </span>
      }
    >
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
    </Row>
  );
}
