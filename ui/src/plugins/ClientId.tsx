import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Dialog } from '../components/Dialog';
import { CopyButton } from '../settings/SignIn';

/** The Client ID and secret as typed. */
export interface Client {
  id: string;
  secret: string;
}

export const noClient: Client = { id: '', secret: '' };

/** As the API takes them: the secret only when one was typed. */
export function given(client: Client) {
  return { client_id: client.id.trim(), client_secret: client.secret.trim() || undefined };
}

interface FieldsProps {
  /** The address the browser comes back to, for the admin to give the app they make. */
  redirect: string;
  client: Client;
  onChange: (client: Client) => void;
}

/**
 * For a tool whose sign-in service takes no app it doesn't know: the address its admin gives the app they make for
 * Gathr, and the Client ID and secret they send back.
 */
export function ClientFields({ redirect, client, onChange }: FieldsProps) {
  return (
    <>
      <div className="agent-field">
        <span className="tool-label">
          Redirect address <small>Redirect URI</small>
        </span>
        <div className="tool-redirect">
          <code>{redirect}</code>
          <CopyButton text={redirect} label="Copy" small />
        </div>
      </div>
      <label className="agent-field">
        Client ID
        <input
          className="tool-link"
          value={client.id}
          onChange={(e) => onChange({ ...client, id: e.target.value })}
          autoComplete="off"
          spellCheck={false}
          // biome-ignore lint/a11y/noAutofocus: the field appears when the tool needs a Client ID.
          autoFocus
        />
      </label>
      <label className="agent-field">
        <span className="tool-label">
          Client secret <small>Optional</small>
        </span>
        <input
          type="password"
          value={client.secret}
          onChange={(e) => onChange({ ...client, secret: e.target.value })}
          autoComplete="off"
        />
      </label>
      <p className="muted">Both stay in Gathr. Agents never see them.</p>
    </>
  );
}

interface ChangeProps {
  redirect: string;
  opener: HTMLElement | null;
  onClose: () => void;
  /** Signs in again with the new ones: they're kept once it's signed in. */
  onSignIn: (client: Client) => void;
}

/** Change Client ID: as Change key, with the address the admin's app must have, as it is now. */
export function ChangeClientId({ redirect, opener, onClose, onSignIn }: ChangeProps) {
  const [client, setClient] = useState(noClient);
  return (
    <Dialog
      open
      title="Change Client ID"
      onClose={onClose}
      opener={opener}
      bare
      className="change-key change-client"
    >
      <form
        className="tool-form"
        onSubmit={(e) => {
          e.preventDefault();
          onSignIn(client);
        }}
      >
        <ClientFields redirect={redirect} client={client} onChange={setClient} />
        <footer className="agent-actions">
          <Button type="button" size="sm" variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" size="sm" disabled={!client.id.trim()}>
            Sign in
          </Button>
        </footer>
      </form>
    </Dialog>
  );
}
