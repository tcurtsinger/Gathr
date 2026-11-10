import { useCallback, useEffect, useId, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Engine, type Failure } from '../api';
import { FailureNotice } from '../components/Notice';
import { Accounts, engines } from './Accounts';
import './engines.css';

type Open = (ref: string) => void;

/** Settings > Engines (settings-entry/v5): the accounts, then each engine's program once. */
export function Engines({ onReference }: { onReference: Open }) {
  return (
    <section aria-labelledby="engines-title">
      <Accounts onReference={onReference} />
      <div className="engine-list">
        <h3>Engines</h3>
        {engines.map(([id, name]) => (
          <Program key={id} id={id} name={name} onReference={onReference} />
        ))}
      </div>
    </section>
  );
}

/** Gathr's copy of one engine: its version and the release check (0025). */
function Program({ id, name, onReference }: { id: string; name: string; onReference: Open }) {
  const [engine, setEngine] = useState<Engine | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const title = useId();

  const load = useCallback(() => api.engine(id).then(setEngine, setFailure), [id]);
  useEffect(() => {
    load();
  }, [load]);
  // A check sends no live event: look again after each answer, until one says it ended.
  const checking = engine?.copy.state === 'checking';
  useEffect(() => {
    if (engine?.copy.state !== 'checking') return;
    const again = setTimeout(load, 1000);
    return () => clearTimeout(again);
  }, [engine, load]);

  const check = () => {
    setFailure(null);
    api.checkEngine(id).then(load, setFailure);
  };

  const copy = engine?.copy;
  const shown = failure ?? (copy?.failed && { error: 'Update failed', ...copy.failed });
  const status = checking
    ? 'Checking'
    : copy?.state === 'off'
      ? 'Updates disabled'
      : copy?.checked && copy.version && !copy.failed
        ? 'Up to date'
        : '';
  return (
    <>
      {shown && <FailureNotice failure={shown} onReference={onReference} />}
      {engine && (
        <>
          <div className="settingrow">
            <h4 id={title}>{name}</h4>
            <span className="muted">{copy?.version ?? (engine.found ? '' : 'Not installed')}</span>
          </div>
          <div className="settingrow">
            <span className="muted" role="status">
              {status}
            </span>
            {copy?.state !== 'off' && (
              <Button
                type="button"
                variant="outline"
                aria-describedby={title}
                disabled={checking}
                onClick={check}
              >
                Check
              </Button>
            )}
          </div>
          {engine.why_not_found && <p className="field-error">{engine.why_not_found}</p>}
        </>
      )}
    </>
  );
}
