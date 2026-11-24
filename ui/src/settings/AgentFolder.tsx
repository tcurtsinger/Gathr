import { X } from 'lucide-react';
import { type FormEvent, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure } from '../api';
import { useCard } from '../components/Corner';
import { IconButton } from '../components/IconButton';
import { desktop, pickFolder } from '../desktop';
import { report } from '../report';
import { Row } from './Row';

/** Settings > Teamwork's Agent folder (M1): every project's Shared folder and one folder per agent, in one place. */
export function AgentFolder() {
  const [folder, setFolder] = useState<string | null>(null);
  // What the person typed while editing; null when not editing. A refused folder stays as typed.
  const [typed, setTyped] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  const change = useRef<HTMLButtonElement>(null);
  const returnFocus = useRef(false);
  const picking = useRef(false);

  useEffect(() => {
    api.settings().then((s) => setFolder(s.agent_folder), setFailure);
  }, []);

  useEffect(() => {
    if (typed === null && returnFocus.current) change.current?.focus();
    returnFocus.current = false;
  }, [typed]);

  const close = () => {
    returnFocus.current = true;
    setTyped(null);
    setFailure(null);
  };

  const store = (path: string) => {
    setSaving(true);
    setFailure(null);
    api
      .saveSettings({ agent_folder: path })
      .then((s) => {
        setFolder(s.agent_folder);
        close();
      }, setFailure)
      .finally(() => setSaving(false));
  };

  const save = (e: FormEvent) => {
    e.preventDefault();
    if (typed?.trim() && !saving) store(typed.trim());
  };

  /** The system's folder picker in the desktop window; a browser, or a picker that fails, gets the field. */
  const pick = () => {
    // One picker at a time: each open is its own system dialog, and Change can still be invoked behind it.
    if (saving || picking.current) return;
    if (!desktop()) return setTyped(folder);
    picking.current = true;
    pickFolder('Agent folder', folder)
      .then(
        (picked) => {
          // Back to Change, as from the field: WebKit never focuses a clicked button.
          change.current?.focus();
          if (picked) store(picked);
        },
        (e) => {
          report({ name: 'gathr.ui.window.folder', error: 'open', attrs: { message: String(e) } });
          setTyped(folder);
        },
      )
      .finally(() => {
        picking.current = false;
      });
  };

  if (folder === null) return null;
  const editor = typed !== null && (
    <form
      className="agent-folder-edit"
      onSubmit={save}
      onKeyDown={(e) => {
        if (e.key !== 'Escape') return;
        e.preventDefault();
        e.stopPropagation();
        close();
      }}
    >
      <span className="pathline">
        <input
          id="agent-folder"
          value={typed}
          autoComplete="off"
          spellCheck={false}
          aria-describedby="agent-folder-scope"
          aria-invalid={failure !== null}
          // biome-ignore lint/a11y/noAutofocus: Change opens the field to type the folder.
          autoFocus
          onFocus={(e) => e.currentTarget.select()}
          onChange={(e) => setTyped(e.target.value)}
        />
        <Button type="submit" disabled={saving || !typed.trim()}>
          {saving ? 'Saving' : 'Save'}
        </Button>
        <IconButton type="button" aria-label="Cancel" className="settings-more" onClick={close}>
          <X />
        </IconButton>
      </span>
      <small id="agent-folder-scope">Applies to new work only.</small>
    </form>
  );
  return (
    <Row
      name={typed === null ? 'Agent folder' : <label htmlFor="agent-folder">Agent folder</label>}
      line="Each project gets a Shared folder and one per agent."
      after={editor}
    >
      {typed === null && (
        <>
          <code className="agent-folder-now path-tail">
            <span>{folder}</span>
          </code>
          <Button type="button" variant="outline" ref={change} aria-busy={saving} onClick={pick}>
            Change
          </Button>
        </>
      )}
    </Row>
  );
}
