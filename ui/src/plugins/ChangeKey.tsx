import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure, type WebTool } from '../api';
import { Dialog } from '../components/Dialog';

interface Props {
  tool: WebTool;
  opener: HTMLElement | null;
  onClose: () => void;
}

/** Change key: one password field. Gathr tries the key first and saves it only when the tool takes it. */
export function ChangeKey({ tool, opener, onClose }: Props) {
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  const [why, setWhy] = useState<string | null>(null);
  const save = () => {
    setBusy(true);
    setWhy(null);
    api.changeTool(tool.id, { key: key.trim() }).then(onClose, (f: Failure) => {
      setBusy(false);
      setWhy(f.why ?? f.error);
    });
  };
  return (
    <Dialog open title="Change key" onClose={onClose} opener={opener} bare className="change-key">
      <form
        className="tool-form"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <label className="agent-field">
          Key
          <input
            type="password"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            aria-invalid={why ? true : undefined}
            autoComplete="off"
            // biome-ignore lint/a11y/noAutofocus: the dialog opens for this one field.
            autoFocus
          />
        </label>
        {why ? (
          <p className="tool-why">{why}</p>
        ) : (
          <p className="muted">The key stays in Gathr. Agents never see it.</p>
        )}
        <footer className="agent-actions">
          <Button type="button" size="sm" variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" size="sm" disabled={busy || !key.trim()}>
            Save
          </Button>
        </footer>
      </form>
    </Dialog>
  );
}
