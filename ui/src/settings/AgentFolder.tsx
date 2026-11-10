import { type FormEvent, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure } from '../api';
import { Help } from '../components/Help';
import { Notice } from '../components/Notice';

/** Settings > General's Agent folder (M1): every project's Shared folder and one folder per agent, in one place. */
export function AgentFolder({ onReference }: { onReference: (ref: string) => void }) {
  const [folder, setFolder] = useState<string | null>(null);
  // What the person typed while editing; null when not editing. A refused folder stays as typed.
  const [typed, setTyped] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const change = useRef<HTMLButtonElement>(null);
  const returnFocus = useRef(false);

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

  const save = (e: FormEvent) => {
    e.preventDefault();
    if (typed === null || !typed.trim() || saving) return;
    setSaving(true);
    setFailure(null);
    api
      .saveSettings({ agent_folder: typed.trim() })
      .then((s) => {
        setFolder(s.agent_folder);
        close();
      }, setFailure)
      .finally(() => setSaving(false));
  };

  if (folder === null) return null;
  return (
    <div className="settingrow agent-folder">
      <span className="setting-label">
        {typed === null ? <span>Agent folder</span> : <label htmlFor="agent-folder">Agent folder</label>}
        <Help label="About agent folder">
          <p>Each project gets a Shared folder and one folder per agent.</p>
        </Help>
      </span>
      {typed === null ? (
        <span className="agent-folder-now">
          <code title={folder}>{folder}</code>
          <Button type="button" variant="outline" ref={change} onClick={() => setTyped(folder)}>
            Change
          </Button>
        </span>
      ) : (
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
            <Button type="button" variant="ghost" onClick={close}>
              Cancel
            </Button>
            <Button type="submit" disabled={saving || !typed.trim()}>
              {saving ? 'Saving' : 'Save'}
            </Button>
          </span>
          <small id="agent-folder-scope">Applies to new work. Current work stays where it is.</small>
        </form>
      )}
      {failure && (
        <Notice
          danger
          title={failure.error}
          reference={failure.ref ? { ref: failure.ref, open: onReference } : undefined}
          links={failure.why && <span className="muted">{failure.why}</span>}
        />
      )}
    </div>
  );
}
