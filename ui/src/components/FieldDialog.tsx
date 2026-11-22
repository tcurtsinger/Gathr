import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import type { Failure } from '../api';
import { Dialog } from './Dialog';

interface Props {
  title: string;
  label: string;
  /** The button that saves: Save, Rename. */
  verb: string;
  /** What the field holds as it opens, all of it selected. */
  initial?: string;
  password?: boolean;
  maxLength?: number;
  /** Under the field, until a save is refused: then why takes its place. */
  note?: string;
  opener: HTMLElement | null;
  onClose: () => void;
  /** Saves the trimmed value; a refusal keeps the dialog open with why. */
  onSave: (value: string) => Promise<unknown>;
}

/** A small dialog with one field, Cancel and the verb: Change key, Rename chat. Enter saves. */
export function FieldDialog({
  title,
  label,
  verb,
  initial = '',
  password,
  maxLength,
  note,
  opener,
  onClose,
  onSave,
}: Props) {
  const [value, setValue] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [why, setWhy] = useState<string | null>(null);
  useEffect(() => () => opener?.focus(), [opener]);
  const save = () => {
    setBusy(true);
    setWhy(null);
    onSave(value.trim()).then(onClose, (f: Failure) => {
      setBusy(false);
      setWhy(f.why ?? f.error);
    });
  };
  return (
    <Dialog open title={title} onClose={onClose} opener={opener} bare className="change-key">
      <form
        className="tool-form"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <label className="agent-field">
          {label}
          <input
            type={password ? 'password' : undefined}
            value={value}
            maxLength={maxLength}
            onChange={(e) => setValue(e.target.value)}
            onFocus={(e) => e.currentTarget.select()}
            aria-invalid={why ? true : undefined}
            autoComplete="off"
            spellCheck={false}
            autoFocus
          />
        </label>
        {why ? <p className="tool-why">{why}</p> : note && <p className="muted">{note}</p>}
        <footer className="agent-actions">
          <Button type="button" size="sm" variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" size="sm" disabled={busy || !value.trim()}>
            {verb}
          </Button>
        </footer>
      </form>
    </Dialog>
  );
}
