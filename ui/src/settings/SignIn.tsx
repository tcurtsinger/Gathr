import { useId, useState } from 'react';
import { type Account, api, type Failure } from '../api';
import { Notice } from '../components/Notice';

type Fail = (f: Failure) => void;
type Waiting = Extract<NonNullable<Account['sign_in']>, { state: 'waiting' }>;
type Failed = Extract<NonNullable<Account['sign_in']>, { state: 'failed' }>;

/** Engines whose sign-in also offers a device code, for signing in from another computer (0033). */
const devices = new Set(['codex']);

const ENABLE = 'Enable device code login in ChatGPT security settings, or ask your workspace admin.';

/** A waiting sign-in under its account (codex-signin/v1): a pasted code, a device code, or the page. */
export function SignInWaiting({
  account,
  waiting,
  onFailure,
}: {
  account: Account;
  waiting: Waiting;
  onFailure: Fail;
}) {
  if (!waiting.url) return null;
  if (waiting.paste) return <SignInCode account={account.id} url={waiting.url} onFailure={onFailure} />;
  if (waiting.user_code)
    return (
      <DeviceCode account={account.id} url={waiting.url} code={waiting.user_code} onFailure={onFailure} />
    );
  const device = devices.has(account.engine);
  return (
    <div className="signin-panel">
      <div className="signin-actions">
        <OpenPage url={waiting.url} />
        {device && (
          <button type="button" onClick={() => api.signIn(account.id, true).catch(onFailure)}>
            Use a device code
          </button>
        )}
      </div>
      {device && (
        <>
          <p className="muted">Finish sign-in on the computer running Gathr.</p>
          <p className="muted">
            On another computer? Use a device code (beta). Enable it in ChatGPT security settings first.
          </p>
        </>
      )}
    </div>
  );
}

/** A failed sign-in: why, with its reference; a refused device code also offers Retry and the browser. */
export function SignInFailed({
  account,
  failed,
  onFailure,
  onReference,
}: {
  account: string;
  failed: Failed;
  onFailure: Fail;
  onReference: (ref: string) => void;
}) {
  return (
    <div className="signin-panel">
      <Notice
        danger
        title="Sign in failed"
        reference={{ ref: failed.ref, open: onReference }}
        links={<span className="muted">{failed.why}</span>}
      />
      {failed.device && (
        <>
          <p className="muted">{ENABLE}</p>
          <div className="signin-actions">
            <button type="button" onClick={() => api.signIn(account, true).catch(onFailure)}>
              Retry
            </button>
            <button type="button" onClick={() => api.signIn(account).catch(onFailure)}>
              Use browser
            </button>
          </div>
        </>
      )}
    </div>
  );
}

/** The sign-in page in the person's browser, for when it didn't open there by itself. */
function OpenPage({ url }: { url: string }) {
  return (
    <button type="button" className="primary" onClick={() => window.open(url, '_blank', 'noreferrer')}>
      Open sign-in page
    </button>
  );
}

/** A device code: the person enters it on the engine's page, in any browser. */
function DeviceCode({
  account,
  url,
  code,
  onFailure,
}: {
  account: string;
  url: string;
  code: string;
  onFailure: Fail;
}) {
  const [copied, setCopied] = useState(false);
  const copy = () => navigator.clipboard.writeText(code).then(() => setCopied(true));
  return (
    <div className="signin-panel">
      <p>
        <strong>Sign in with a device code</strong> <span className="muted">(beta)</span>
      </p>
      <p className="muted">Enter this code on the sign-in page. Any browser works.</p>
      <div className="signin-actions">
        <code className="device-code">{code}</code>
        <button type="button" onClick={copy}>
          {copied ? 'Copied' : 'Copy code'}
        </button>
      </div>
      <div className="signin-actions">
        <OpenPage url={url} />
        <button type="button" onClick={() => api.signIn(account).catch(onFailure)}>
          Use browser
        </button>
      </div>
      <p className="muted">{ENABLE}</p>
      <p className="muted" role="status">
        Waiting for sign-in
      </p>
    </div>
  );
}

/** While the engine waits for the code its page gives: that page, and the code. */
function SignInCode({ account, url, onFailure }: { account: string; url: string; onFailure: Fail }) {
  const [code, setCode] = useState('');
  const [sent, setSent] = useState(false);
  const field = useId();
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setSent(true);
    api.signInCode(account, code.trim()).catch((f) => {
      setSent(false);
      onFailure(f);
    });
  };
  return (
    <form className="pathfield" onSubmit={submit}>
      <label htmlFor={field}>Code</label>
      <div className="pathline">
        <input
          id={field}
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
