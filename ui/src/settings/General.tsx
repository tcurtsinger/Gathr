import { useEffect, useRef, useState } from 'react';
import { api, type Failure } from '../api';
import { Notice } from '../components/Notice';
import { Switch } from '../components/Switch';

/** Settings > General (startup/v1): what Gathr does on this computer, whoever its accounts are. */
export function General({ onReference }: { onReference: (ref: string) => void }) {
  const [on, setOn] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  // Counts saves started and ended: a read that began before a save ended may answer the old state.
  const saves = useRef(0);

  // Read again on coming back to the window: the person may have changed it in the system's settings.
  useEffect(() => {
    const read = () => {
      const at = saves.current;
      api.startup().then(
        (next) => {
          if (at !== saves.current) return;
          setOn(next);
          if (next) setFailure(null);
        },
        (f: Failure) => at === saves.current && setFailure(f),
      );
    };
    read();
    window.addEventListener('focus', read);
    return () => window.removeEventListener('focus', read);
  }, []);

  /** Shows the change at once; a failure puts the switch back as it was. */
  const change = (next: boolean) => {
    const before = on;
    saves.current++;
    setOn(next);
    setSaving(true);
    setFailure(null);
    api
      .setStartup(next)
      .then(setOn, (f: Failure) => {
        setOn(before);
        setFailure(f);
      })
      .finally(() => {
        saves.current++;
        setSaving(false);
      });
  };

  return (
    <section aria-labelledby="general-title">
      <h3 id="general-title">General</h3>
      {failure && (
        <Notice
          danger
          title={failure.error}
          reference={failure.ref ? { ref: failure.ref, open: onReference } : undefined}
          links={failure.why && <span className="muted">{failure.why}</span>}
        />
      )}
      {on !== null && (
        <div className="settingrow">
          <span id="startup-label">Open at login</span>
          <span className="setting-scope" id="startup-scope">
            This computer
          </span>
          <span className="visually-hidden" role="status">
            {saving ? 'Saving' : ''}
          </span>
          <Switch
            on={on}
            labelledBy="startup-label"
            describedBy="startup-scope"
            disabled={saving}
            onChange={change}
          />
        </div>
      )}
    </section>
  );
}
