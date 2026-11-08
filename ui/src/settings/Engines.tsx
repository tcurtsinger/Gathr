import { useCallback, useEffect, useState } from 'react';
import { type Account, api, type Engine, type Failure } from '../api';
import { Notice } from '../components/Notice';
import { listen } from '../live';
import './engines.css';

const said: Record<string, string> = {
  signed_in: 'Signed in',
  signed_out: 'Signed out',
  stale: 'Signed in',
  unknown: 'Checking',
};

/** Chats run on this account until agents pick theirs; the accounts list is settings-entry/v5. */
const CHAT_ACCOUNT = 'main';

/** Settings > Engines: Claude Code and the chats' account's sign-in. */
export function Engines({ onReference }: { onReference: (ref: string) => void }) {
  const [engine, setEngine] = useState<Engine | null>(null);
  const [account, setAccount] = useState<Account | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);

  const load = useCallback(
    () =>
      Promise.all([api.engine(), api.account(CHAT_ACCOUNT)]).then(([e, a]) => {
        setEngine(e);
        setAccount(a);
      }, setFailure),
    [],
  );
  useEffect(() => {
    load();
    return listen((event) => {
      if (event.type === 'accounts' || event.type === 'lagged') load();
    });
  }, [load]);

  const signIn = () => {
    setFailure(null);
    api.signIn(CHAT_ACCOUNT).then(load, setFailure);
  };

  const signingIn = account?.sign_in?.state === 'waiting' ? account.sign_in : null;
  const signInFailed = account?.sign_in?.state === 'failed' ? account.sign_in : null;
  return (
    <section aria-labelledby="engines-title">
      <h3 id="engines-title">Engines</h3>
      {failure && (
        <Notice
          danger
          title={failure.error}
          reference={failure.ref ? { ref: failure.ref, open: onReference } : undefined}
          links={failure.why && <span className="muted">{failure.why}</span>}
        />
      )}
      {signInFailed && (
        <Notice
          danger
          title="Sign in failed"
          reference={{ ref: signInFailed.ref, open: onReference }}
          links={<span className="muted">{signInFailed.why}</span>}
        />
      )}
      {engine && account && (
        <>
          <div className="settingrow">
            <h4>Claude Code</h4>
            <span className="rowend">
              <span className="muted">
                {signingIn ? 'Signing in' : (said[account.login] ?? account.login)}
              </span>
              {signingIn ? (
                <button type="button" onClick={() => api.cancelSignIn(CHAT_ACCOUNT).then(load, setFailure)}>
                  Cancel
                </button>
              ) : (
                account.login === 'signed_out' && (
                  <button type="button" className="primary" onClick={signIn}>
                    Sign in
                  </button>
                )
              )}
            </span>
          </div>
          {signingIn?.url && <SignInCode url={signingIn.url} onFailure={setFailure} />}
          {engine.why_not_found && <p className="field-error">{engine.why_not_found}</p>}
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
    api.signInCode(CHAT_ACCOUNT, code.trim()).catch((f) => {
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
