import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Account, api, type Failure } from '../api';
import { ConfirmRemove } from '../components/ConfirmRemove';
import { FailureNotice } from '../components/Notice';
import { listen } from '../live';
import { SignInFailed, SignInWaiting } from './SignIn';

/** The engines Gathr can run, as the service names them. */
export const engines: [string, string][] = [
  ['claude', 'Claude Code'],
  ['codex', 'Codex'],
];
export const engineName = (engine: string) => engines.find(([id]) => id === engine)?.[1] ?? engine;

const said: Record<string, string> = {
  signed_in: 'Signed in',
  signed_out: 'Signed out',
  stale: 'Signed in',
  unknown: 'Checking',
};

type Open = (ref: string) => void;

/** Settings > Engines' accounts (settings-entry/v5): each named, on one engine, signed in on its own. */
export function Accounts({ onReference }: { onReference: Open }) {
  const [accounts, setAccounts] = useState<Account[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  /** The form, open to add (no account) or to rename one. */
  const [form, setForm] = useState<{ account?: Account; opener: HTMLElement } | null>(null);
  /** Gets focus once an account is removed, as its row goes. */
  const addButton = useRef<HTMLButtonElement>(null);

  const load = useCallback(() => api.accounts().then(setAccounts, setFailure), []);
  useEffect(() => {
    load();
    return listen((event) => {
      if (event.type === 'accounts' || event.type === 'lagged') load();
    });
  }, [load]);

  const open = (opener: HTMLElement, account?: Account) => {
    setFailure(null);
    setForm({ account, opener });
  };
  const close = () => {
    form?.opener.focus();
    setForm(null);
  };

  return (
    <>
      <div className="account-heading">
        <h3 id="engines-title">Accounts</h3>
        <Button type="button" variant="outline" ref={addButton} onClick={(e) => open(e.currentTarget)}>
          Add
        </Button>
      </div>
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
      {form && (
        // A new form for each account, so one account's draft never carries to another.
        <AccountForm
          key={form.account?.id ?? ''}
          account={form.account}
          onClose={close}
          onFailure={setFailure}
        />
      )}
      {accounts?.length === 0 && <p className="muted">No accounts</p>}
      <ul className="account-list">
        {accounts?.map((account) => (
          <AccountRow
            key={account.id}
            account={account}
            onRename={(opener) => open(opener, account)}
            onRemoved={() => addButton.current?.focus()}
            onFailure={setFailure}
            onReference={onReference}
          />
        ))}
      </ul>
    </>
  );
}

interface FormProps {
  account?: Account;
  onClose: () => void;
  onFailure: (f: Failure) => void;
}

/** Add asks for the engine and a name, then signs in; Rename keeps the engine. */
function AccountForm({ account, onClose, onFailure }: FormProps) {
  const [engine, setEngine] = useState(account?.engine ?? engines[0][0]);
  const [name, setName] = useState(account?.name ?? '');
  const [busy, setBusy] = useState(false);
  const title = useId();

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    const failed = (f: Failure) => {
      setBusy(false);
      onFailure(f);
    };
    if (account) {
      api.renameAccount(account.id, name.trim()).then(onClose, failed);
      return;
    }
    api.addAccount(engine, name.trim()).then((added) => {
      onClose();
      api.signIn(added.id).catch(onFailure);
    }, failed);
  };

  return (
    <form className="account-form" aria-labelledby={title} onSubmit={submit}>
      <h4 id={title}>{account ? 'Rename account' : 'Add account'}</h4>
      <label>
        Engine
        <select value={engine} disabled={!!account} onChange={(e) => setEngine(e.target.value)}>
          {engines.map(([id, label]) => (
            <option key={id} value={id}>
              {label}
            </option>
          ))}
        </select>
      </label>
      <label>
        Name
        <input
          value={name}
          maxLength={80}
          autoComplete="off"
          spellCheck={false}
          // biome-ignore lint/a11y/noAutofocus: the form opens from Add or Rename, to type the name
          autoFocus
          onChange={(e) => setName(e.target.value)}
        />
      </label>
      <div className="account-form-actions">
        <Button type="button" variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button type="submit" disabled={busy || !name.trim()}>
          {account ? 'Save' : 'Sign in'}
        </Button>
      </div>
    </form>
  );
}

interface RowProps {
  account: Account;
  onRename: (opener: HTMLElement) => void;
  onRemoved: () => void;
  onFailure: (f: Failure) => void;
  onReference: Open;
}

function AccountRow({ account, onRename, onRemoved, onFailure, onReference }: RowProps) {
  const [busy, setBusy] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  /** Focus goes back to Remove when its question is cancelled. */
  const cancelled = useRef(false);
  const name = useId();
  const id = account.id;
  const waiting = account.sign_in?.state === 'waiting' ? account.sign_in : null;
  const failed = account.sign_in?.state === 'failed' ? account.sign_in : null;
  const state = busy ?? (waiting ? 'Signing in' : (said[account.login] ?? account.login));

  const act = (doing: string, call: Promise<unknown>) => {
    setBusy(doing);
    call.catch(onFailure).finally(() => setBusy(null));
  };
  const action = waiting ? (
    <Button
      type="button"
      variant="outline"
      aria-describedby={name}
      onClick={() => act('Cancelling', api.cancelSignIn(id))}
    >
      Cancel
    </Button>
  ) : account.login === 'signed_out' ? (
    <Button
      type="button"
      variant="outline"
      aria-describedby={name}
      onClick={() => act('Signing in', api.signIn(id))}
    >
      Sign in
    </Button>
  ) : (
    account.login !== 'unknown' && (
      <Button
        type="button"
        variant="outline"
        aria-describedby={name}
        onClick={() => act('Signing out', api.signOut(id))}
      >
        Sign out
      </Button>
    )
  );

  return (
    <li className="account-row">
      <div>
        <strong id={name}>{account.name}</strong>
        <small>
          {engineName(account.engine)} / {state}
        </small>
      </div>
      {confirming ? (
        <ConfirmRemove
          name={account.name}
          onCancel={() => {
            cancelled.current = true;
            setConfirming(false);
          }}
          onRemove={() => {
            setConfirming(false);
            act('Removing', api.removeAccount(id).then(onRemoved));
          }}
        />
      ) : (
        <div className="account-actions">
          <Button
            type="button"
            variant="outline"
            aria-describedby={name}
            onClick={(e) => onRename(e.currentTarget)}
          >
            Rename
          </Button>
          {!busy && action}
          {!busy && (
            <Button
              type="button"
              variant="outline"
              aria-describedby={name}
              ref={(b) => {
                if (b && cancelled.current) b.focus();
                cancelled.current = false;
              }}
              onClick={() => setConfirming(true)}
            >
              Remove
            </Button>
          )}
        </div>
      )}
      {failed && (
        <SignInFailed account={id} failed={failed} onFailure={onFailure} onReference={onReference} />
      )}
      {waiting && <SignInWaiting account={account} waiting={waiting} onFailure={onFailure} />}
    </li>
  );
}
