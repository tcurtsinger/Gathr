import { useCallback, useEffect, useState } from 'react';
import { api, type Engine, type Failure } from '../api';
import { Notice } from '../components/Notice';
import { Accounts } from './Accounts';
import './engines.css';

/** Settings > Engines (settings-entry/v5): the accounts, then each engine's program once. */
export function Engines({ onReference }: { onReference: (ref: string) => void }) {
  return (
    <section aria-labelledby="engines-title">
      <Accounts onReference={onReference} />
      <Programs onReference={onReference} />
    </section>
  );
}

/** Gathr's copy of Claude Code: its version and the release check (0025). */
function Programs({ onReference }: { onReference: (ref: string) => void }) {
  const [engine, setEngine] = useState<Engine | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);

  const load = useCallback(() => api.engine().then(setEngine, setFailure), []);
  useEffect(() => {
    load();
  }, [load]);
  // A check sends no live event: look again until it ends.
  const checking = engine?.copy.state === 'checking';
  useEffect(() => {
    if (!checking) return;
    const again = setTimeout(load, 1000);
    return () => clearTimeout(again);
  }, [checking, load]);

  const check = () => {
    setFailure(null);
    api.checkEngine().then(load, setFailure);
  };

  const copy = engine?.copy;
  const shown = failure ?? (copy?.failed && { error: 'Update failed', ...copy.failed });
  const status = checking
    ? 'Checking'
    : copy?.state === 'off'
      ? 'Updates off'
      : copy?.checked
        ? 'Up to date'
        : '';
  return (
    <div className="engine-list">
      <h3>Engines</h3>
      {shown && (
        <Notice
          danger
          title={shown.error}
          reference={shown.ref ? { ref: shown.ref, open: onReference } : undefined}
          links={shown.why && <span className="muted">{shown.why}</span>}
        />
      )}
      {engine && (
        <>
          <div className="settingrow">
            <h4>Claude Code</h4>
            <span className="muted">{copy?.version ?? (engine.found ? '' : 'Not installed')}</span>
          </div>
          <div className="settingrow">
            <span className="muted" role="status">
              {status}
            </span>
            {copy?.state !== 'off' && (
              <button type="button" disabled={checking} onClick={check}>
                Check
              </button>
            )}
          </div>
          {engine.why_not_found && <p className="field-error">{engine.why_not_found}</p>}
        </>
      )}
    </div>
  );
}
