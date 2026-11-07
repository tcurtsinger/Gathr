import { useCallback, useEffect, useState } from 'react';
import { api, type Engine, type Failure } from '../api';
import { Notice } from '../components/Notice';
import './engines.css';

const said: Record<string, string> = {
  signed_in: 'Signed in',
  signed_out: 'Signed out',
  stale: 'Signed in',
  unknown: 'Checking',
};

/** Settings > Engines: Claude Code's sign-in and path (engines/v4). */
export function Engines() {
  const [engine, setEngine] = useState<Engine | null>(null);
  const [path, setPath] = useState('');
  const [failure, setFailure] = useState<Failure | null>(null);
  const [saved, setSaved] = useState(false);

  const load = useCallback(
    () =>
      api.engine().then((e) => {
        setEngine(e);
        setPath(e.path ?? '');
      }, setFailure),
    [],
  );
  useEffect(() => {
    load();
  }, [load]);

  const save = () => {
    setSaved(false);
    api.setEnginePath(path.trim() || null).then(() => {
      setSaved(true);
      setFailure(null);
      load();
    }, setFailure);
  };

  return (
    <section aria-labelledby="engines-title">
      <h3 id="engines-title">Engines</h3>
      {failure && (
        <Notice
          danger
          title={failure.error}
          reference={failure.ref ? { ref: failure.ref, open: () => {} } : undefined}
        />
      )}
      {engine && (
        <>
          <div className="settingrow">
            <h4>Claude Code</h4>
            <span className="muted">{said[engine.account.login] ?? engine.account.login}</span>
          </div>
          <div className="pathfield">
            <label htmlFor="claude-path">Claude Code path</label>
            <div className="pathline">
              <input
                id="claude-path"
                value={path}
                placeholder={engine.found ?? 'Not found'}
                onChange={(e) => {
                  setPath(e.target.value);
                  setSaved(false);
                }}
              />
              <button type="button" onClick={save}>
                {saved ? 'Saved' : 'Save'}
              </button>
            </div>
            {engine.why_not_found && <p className="field-error">{engine.why_not_found}</p>}
          </div>
          <details className="settingrow folder">
            <summary>Login folder</summary>
            <code>{engine.account.folder}</code>
          </details>
        </>
      )}
    </section>
  );
}
