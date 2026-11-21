import { ChevronLeft } from 'lucide-react';
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import type { Failure } from '../api';
import { FailureNotice } from '../components/Notice';

interface Props {
  onBack: () => void;
  /** Settles once it's written; a refusal stays on the form. */
  onSave: (skill: { name: string; description: string; text: string }) => Promise<unknown>;
  onReference: (ref: string) => void;
}

/** New skill: its name, what it's for, and its instructions, written as its folder and SKILL.md. */
export function NewSkill({ onBack, onSave, onReference }: Props) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [text, setText] = useState('');
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const save = () => {
    setBusy(true);
    setFailure(null);
    onSave({ name: name.trim(), description: description.trim(), text }).catch((f: Failure) => {
      setBusy(false);
      setFailure(f);
    });
  };
  return (
    <form
      className="new-skill"
      onSubmit={(e) => {
        e.preventDefault();
        save();
      }}
    >
      <button type="button" className="plugins-back" onClick={onBack}>
        <ChevronLeft aria-hidden="true" />
        Plugins
      </button>
      <h1>New skill</h1>
      <label className="agent-field">
        Name
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          spellCheck={false}
          placeholder="waiver-claims"
        />
      </label>
      <label className="agent-field">
        Description
        <input value={description} onChange={(e) => setDescription(e.target.value)} />
      </label>
      <label className="agent-field">
        Instructions
        <textarea value={text} onChange={(e) => setText(e.target.value)} rows={14} />
      </label>
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
      <footer className="agent-actions">
        <Button type="button" size="sm" variant="outline" onClick={onBack}>
          Cancel
        </Button>
        <Button type="submit" size="sm" disabled={busy || !name.trim() || !description.trim()}>
          Save
        </Button>
      </footer>
    </form>
  );
}
