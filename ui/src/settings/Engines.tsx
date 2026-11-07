import { useCallback, useEffect, useState } from 'react';
import { api, type Engine, type Failure } from '../api';
import { Notice } from '../components/Notice';
import { listen } from '../live';
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
  const [folderShown, setFolderShown] = useState(false);

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
    return listen((event) => {
      if (event.type === 'engine' || event.type === 'lagged') load();
    });
  }, [load]);

  const save = () => {
    setSaved(false);
    api.setEnginePath(path.trim() || null).then(() => {
      setSaved(true);
      setFailure(null);
      load();
    }, setFailure);
  };

  const signIn = () => {
    setFailure(null);
    api.signIn().then(load, setFailure);
  };

  const signingIn = engine?.sign_in?.state === 'waiting' ? engine.sign_in : null;
  const signInFailed = engine?.sign_in?.state === 'failed' ? engine.sign_in : null;

  return (
    <section aria-labelledby="engines-title">
      <h3 id="engines-title">Engines</h3>
      {failure && (
        <Notice
          danger
          title={failure.error}
          reference={failure.ref ? { ref: failure.ref, open: () => {} } : undefined}
          links={failure.why && <span className="muted">{failure.why}</span>}
        />
      )}
      {signInFailed && (
        <Notice
          danger
          title="Sign in failed"
          reference={{ ref: signInFailed.ref, open: () => {} }}
          links={<span className="muted">{signInFailed.why}</span>}
        />
      )}
      {engine && (
        <>
          <div className="settingrow">
            <h4>Claude Code</h4>
            <span className="rowend">
              <span className="muted">
                {signingIn ? 'Signing in' : (said[engine.account.login] ?? engine.account.login)}
              </span>
              {signingIn ? (
                <button type="button" onClick={() => api.cancelSignIn().then(load, setFailure)}>
                  Cancel
                </button>
              ) : (
                engine.account.login === 'signed_out' && (
                  <button type="button" className="primary" onClick={signIn}>
                    Sign in
                  </button>
                )
              )}
            </span>
          </div>
          {signingIn && <SignInCode url={signingIn.url} onFailure={setFailure} />}
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
          <details className="folder" onToggle={(e) => setFolderShown(e.currentTarget.open)}>
            <summary>
              Login folder<span>{folderShown ? 'Hide' : 'Show'}</span>
            </summary>
            <code>{engine.account.folder}</code>
          </details>
        </>
      )}
    </section>
  );
}

/** While Claude Code waits: its page, if the browser didn't open, and the code that page gives. */
function SignInCode({ url, onFailure }: { url: string; onFailure: (f: Failure) => void }) {
  const [code, setCode] = useState('');
  const [sent, setSent] = useState(false);
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setSent(true);
    api.signInCode(code.trim()).catch((f) => {
      setSent(false);
      onFailure(f);
    });
  };
  return (
    <form className="pathfield" onSubmit={submit}>
      <label htmlFor="sign-in-code">Code</label>
      <div className="pathline">
        <input
          id="sign-in-code"
          value={code}
          autoComplete="off"
          spellCheck={false}
          onChange={(e) => setCode(e.target.value)}
        />
        <button type="submit" disabled={sent || !code.trim()}>
          {sent ? 'Signing in' : 'Sign in'}
        </button>
      </div>
      <a href={url} target="_blank" rel="noreferrer">
        Open sign-in page
      </a>
    </form>
  );
}
