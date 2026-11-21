import { useId, useMemo, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Account, api, Failure } from '../api';
import { useCard } from '../components/Corner';

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
          <Button
            type="button"
            variant="outline"
            onClick={() => api.signIn(account.id, true).catch(onFailure)}
          >
            Use a device code
          </Button>
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

/** A failed sign-in, told in the corner; a refused device code also offers Retry there, and the browser here. */
export function SignInFailed({
  account,
  failed,
  onFailure,
}: {
  account: string;
  failed: Failed;
  onFailure: Fail;
}) {
  const { why, ref, device } = failed;
  useCard(
    useMemo(() => new Failure('Sign-in failed', why, ref), [why, ref]),
    device ? { label: 'Retry', run: () => api.signIn(account, true).catch(onFailure) } : undefined,
  );
  if (!device) return null;
  return (
    <div className="signin-panel">
      <p className="muted">{ENABLE}</p>
      <div className="signin-actions">
        <Button type="button" variant="outline" onClick={() => api.signIn(account).catch(onFailure)}>
          Use browser
        </Button>
      </div>
    </div>
  );
}

/** The sign-in page in the person's browser, for when it didn't open there by itself. */
export function OpenPage({ url, small }: { url: string; small?: boolean }) {
  return (
    <Button
      type="button"
      variant={small ? 'outline' : undefined}
      size={small ? 'sm' : undefined}
      onClick={() => window.open(url, '_blank', 'noreferrer')}
    >
      Open sign-in page
    </Button>
  );
}

/** Copies `text` for the person to paste elsewhere, and says so once it has. */
export function CopyButton({ text, label, small }: { text: string; label: string; small?: boolean }) {
  const [copied, setCopied] = useState(false);
  const copy = () => navigator.clipboard.writeText(text).then(() => setCopied(true));
  return (
    <Button type="button" variant="outline" size={small ? 'sm' : undefined} onClick={copy}>
      {copied ? 'Copied' : label}
    </Button>
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
  return (
    <div className="signin-panel">
      <p>
        <strong>Sign in with a device code</strong> <span className="muted">(beta)</span>
      </p>
      <p className="muted">Enter this code on the sign-in page. Any browser works.</p>
      <div className="signin-actions">
        <code className="device-code">{code}</code>
        <CopyButton text={code} label="Copy code" />
      </div>
      <div className="signin-actions">
        <OpenPage url={url} />
        <Button type="button" variant="outline" onClick={() => api.signIn(account).catch(onFailure)}>
          Use browser
        </Button>
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
        <Button type="submit" disabled={sent || !code.trim()}>
          {sent ? 'Signing in' : 'Sign in'}
        </Button>
      </div>
      <a href={url} target="_blank" rel="noreferrer">
        Open sign-in page
      </a>
    </form>
  );
}
