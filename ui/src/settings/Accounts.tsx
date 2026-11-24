import { MoreHorizontal, Pencil, Trash2, X } from 'lucide-react';
import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { type Account, api, downloading, type Failure } from '../api';
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

/** The engines whose program is still downloading: none can sign in or list models until it is there. Looked at
 * again every few seconds while one is, as a download sends no live event. */
export function useDownloading() {
  const [list, setList] = useState<string[]>([]);
  useEffect(() => {
    let again: ReturnType<typeof setTimeout> | undefined;
    let gone = false;
    const look = () =>
      Promise.all(
        engines.map(([id]) => api.engine(id).then((e) => (downloading(e.why_not_found) ? [id] : []))),
      )
        .then((found) => {
          if (gone) return;
          const now = found.flat();
          setList((before) => (before.join() === now.join() ? before : now));
          if (now.length) again = setTimeout(look, 3000);
        })
        .catch(() => {});
    look();
    return () => {
      gone = true;
      clearTimeout(again);
    };
  }, []);
  return list;
}

const said: Record<string, string> = {
  signed_in: 'Signed in',
  signed_out: 'Signed out',
  stale: 'Signed in',
  unknown: 'Checking',
};

/** Settings > Engines' accounts (settings-entry/v5): each named, on one engine, signed in on its own. `adding`: the
 * page's Add was pressed, and a row to name the new account shows under the list. */
export function Accounts({ adding, onAdded }: { adding: boolean; onAdded: () => void }) {
  const [accounts, setAccounts] = useState<Account[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  /** The account being renamed, in its own row. */
  const [renaming, setRenaming] = useState<string | null>(null);
  const fetching = useDownloading();

  const load = useCallback(() => api.accounts().then(setAccounts, setFailure), []);
  useEffect(() => {
    load();
    return listen((event) => {
      if (event.type === 'accounts' || event.type === 'lagged') load();
    });
  }, [load]);

  return (
    <>
      <Group title="Accounts" />
      {accounts?.length === 0 && !adding && <p className="settings-empty">No accounts</p>}
      {((accounts && accounts.length > 0) || adding) && (
        <ul className="settings-rows settings-top">
          {(accounts ?? []).map((account) =>
            renaming === account.id ? (
              <AccountForm
                key={account.id}
                account={account}
                onClose={() => setRenaming(null)}
                onFailure={setFailure}
              />
            ) : (
              <AccountRow
                key={account.id}
                account={account}
                engineWaits={fetching.includes(account.engine)}
                onRename={() => setRenaming(account.id)}
                onFailure={setFailure}
              />
            ),
          )}
          {adding && <AccountForm onClose={onAdded} onFailure={setFailure} />}
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

/** One row to name an account (the owner's pick 5A, 2026-11-24): Add asks for the engine too, then signs in; Rename
 * keeps the engine. */
function AccountForm({ account, onClose, onFailure }: FormProps) {
  const [engine, setEngine] = useState(account?.engine ?? engines[0][0]);
  const [name, setName] = useState(account?.name ?? '');
  const [busy, setBusy] = useState(false);

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
    <li className="account-row">
      <form
        className="setting account-form"
        aria-label={account ? `Rename ${account.name}` : 'Add account'}
        onSubmit={submit}
        onKeyDown={(e) => {
          if (e.key !== 'Escape') return;
          e.preventDefault();
          e.stopPropagation();
          onClose();
        }}
      >
        <div className="setting-words">
          <EngineLogo engine={engine} className="engine-logo" />
          <input
            aria-label="Name"
            placeholder="Account name"
            value={name}
            maxLength={80}
            autoComplete="off"
            spellCheck={false}
            // biome-ignore lint/a11y/noAutofocus: the row opens from Add or Rename, to type the name
            autoFocus
            onChange={(e) => setName(e.target.value)}
          />
        </div>
        <div className="setting-control">
          {!account && (
            <Dropdown
              aria-label="Engine"
              className="account-engine"
              value={engine}
              onChange={setEngine}
              choices={engines.map(([id, label]) => ({ value: id, label }))}
            />
          )}
          <Button type="submit" disabled={busy || !name.trim()}>
            {account ? 'Save' : 'Add'}
          </Button>
          <IconButton type="button" aria-label="Cancel" className="settings-more" onClick={onClose}>
            <X />
          </IconButton>
        </div>
      </form>
    </li>
  );
}

interface RowProps {
  account: Account;
  /** Its engine's program is still downloading: a signed-out account waits for it to sign in. */
  engineWaits: boolean;
  onRename: () => void;
  onFailure: (f: Failure) => void;
}

function AccountRow({ account, engineWaits, onRename, onFailure }: RowProps) {
  const [busy, setBusy] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  /** Focus goes back to the menu's button when Remove's question is cancelled. */
  const cancelled = useRef(false);
  const name = useId();
  const id = account.id;
  const waiting = account.sign_in?.state === 'waiting' ? account.sign_in : null;
  const failed = account.sign_in?.state === 'failed' ? account.sign_in : null;
  const held = engineWaits && account.login === 'signed_out' && !waiting;
  const state = busy ?? (waiting ? 'Signing in' : held ? 'Waiting' : (said[account.login] ?? account.login));

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
      disabled={held}
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
              act('Removing', api.removeAccount(id));
            }}
          />
        ) : (
          <>
            {/* Its own column, so the states line up down the list whatever the engine's name. */}
            <span className="account-state" data-state={busy || waiting || held ? 'busy' : account.login}>
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
  onRename: () => void;
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
        <DropdownMenuItem onSelect={pick(onRename)}>
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
