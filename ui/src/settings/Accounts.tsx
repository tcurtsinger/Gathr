import { MoreHorizontal, Pencil, Plus, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { type Account, api, type Failure } from '../api';
import { ConfirmRemove } from '../components/ConfirmRemove';
import { useCard } from '../components/Corner';
import { DialogMenuContent } from '../components/DialogMenu';
import { Dropdown } from '../components/Dropdown';
import { EngineLogo } from '../components/EngineLogo';
import { IconButton } from '../components/IconButton';
import { listen } from '../live';
import { Group, Row } from './Row';
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

/** Settings > Engines' accounts (settings-entry/v5): each named, on one engine, signed in on its own. */
export function Accounts() {
  const [accounts, setAccounts] = useState<Account[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  /** The form, open to add (no account) or to rename one. */
  const [form, setForm] = useState<{ account?: Account; opener: HTMLElement | null } | null>(null);
  /** Gets focus once an account is removed, as its row goes. */
  const addButton = useRef<HTMLButtonElement>(null);

  const load = useCallback(() => api.accounts().then(setAccounts, setFailure), []);
  useEffect(() => {
    load();
    return listen((event) => {
      if (event.type === 'accounts' || event.type === 'lagged') load();
    });
  }, [load]);

  const open = (opener: HTMLElement | null, account?: Account) => {
    setFailure(null);
    setForm({ account, opener });
  };
  const close = () => {
    form?.opener?.focus();
    setForm(null);
  };

  return (
    <>
      <Group
        title="Accounts"
        right={
          <Button type="button" variant="outline" ref={addButton} onClick={(e) => open(e.currentTarget)}>
            <Plus />
            Add
          </Button>
        }
      />
      {form && (
        // A new form for each account, so one account's draft never carries to another.
        <AccountForm
          key={form.account?.id ?? ''}
          account={form.account}
          onClose={close}
          onFailure={setFailure}
        />
      )}
      {accounts?.length === 0 && <p className="settings-empty">No accounts</p>}
      {accounts && accounts.length > 0 && (
        <ul className="settings-rows settings-top">
          {accounts.map((account) => (
            <AccountRow
              key={account.id}
              account={account}
              onRename={(opener) => open(opener, account)}
              onRemoved={() => addButton.current?.focus()}
              onFailure={setFailure}
            />
          ))}
        </ul>
      )}
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
      <h5 id={title}>{account ? 'Rename account' : 'Add account'}</h5>
      <div className="account-field">
        <label htmlFor={`${title}engine`}>Engine</label>
        <Dropdown
          id={`${title}engine`}
          value={engine}
          disabled={!!account}
          onChange={setEngine}
          choices={engines.map(([id, label]) => ({
            value: id,
            label,
            icon: <EngineLogo engine={id} className="engine-choice-logo" />,
          }))}
        />
      </div>
      <label className="account-field">
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
  onRename: (opener: HTMLElement | null) => void;
  onRemoved: () => void;
  onFailure: (f: Failure) => void;
}

function AccountRow({ account, onRename, onRemoved, onFailure }: RowProps) {
  const [busy, setBusy] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  /** Focus goes back to the menu's button when Remove's question is cancelled. */
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
    <li className="account-row" data-confirming={confirming || undefined}>
      <Row
        name={<span id={name}>{account.name}</span>}
        line={engineName(account.engine)}
        lead={<EngineLogo engine={account.engine} className="engine-logo" />}
        after={
          <>
            {failed && <SignInFailed account={id} failed={failed} onFailure={onFailure} />}
            {waiting && <SignInWaiting account={account} waiting={waiting} onFailure={onFailure} />}
          </>
        }
      >
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
          <>
            {/* Its own column, so the states line up down the list whatever the engine's name. */}
            <span className="account-state" data-state={busy || waiting ? 'busy' : account.login}>
              {state}
            </span>
            <span className="account-action">{!busy && action}</span>
            <AccountMenu
              account={account}
              busy={busy !== null}
              focus={cancelled}
              onRename={onRename}
              onRemove={() => setConfirming(true)}
            />
          </>
        )}
      </Row>
    </li>
  );
}

/** An account's More: Rename, then Remove (not while something is under way), as a tool's More in Plugins. */
function AccountMenu({
  account,
  busy,
  focus,
  onRename,
  onRemove,
}: {
  account: Account;
  busy: boolean;
  /** Set when Remove's question was cancelled: its button takes focus back as it shows again. */
  focus: React.RefObject<boolean>;
  onRename: (opener: HTMLElement | null) => void;
  onRemove: () => void;
}) {
  const trigger = useRef<HTMLButtonElement | null>(null);
  const [host, setHost] = useState<HTMLElement | null>(null);
  const place = useCallback(
    (b: HTMLButtonElement | null) => {
      trigger.current = b;
      setHost(b?.closest('dialog') ?? null);
      if (b && focus.current) b.focus();
      focus.current = false;
    },
    [focus],
  );
  // What was picked opens its own form or question, which takes focus: it opens once the menu has closed, in place
  // of focus going back to the menu's button.
  const picked = useRef<(() => void) | null>(null);
  const pick = (then: () => void) => () => {
    picked.current = then;
  };
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <IconButton ref={place} aria-label={`More for ${account.name}`} className="settings-more">
          <MoreHorizontal />
        </IconButton>
      </DropdownMenuTrigger>
      <DialogMenuContent
        host={host}
        align="end"
        sideOffset={8}
        onCloseAutoFocus={(e) => {
          if (!picked.current) return;
          e.preventDefault();
          picked.current();
          picked.current = null;
        }}
      >
        <DropdownMenuItem onSelect={pick(() => onRename(trigger.current))}>
          <Pencil />
          Rename
        </DropdownMenuItem>
        {!busy && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem variant="destructive" onSelect={pick(onRemove)}>
              <Trash2 />
              Remove
            </DropdownMenuItem>
          </>
        )}
      </DialogMenuContent>
    </DropdownMenu>
  );
}
