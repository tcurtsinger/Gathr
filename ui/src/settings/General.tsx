import { useEffect, useRef, useState } from 'react';
import { api, type Failure } from '../api';
import { FailureNotice } from '../components/Notice';
import { Switch } from '../components/Switch';
import { useNewestWins } from '../newest';
import { savedTheme, setTheme, type Theme } from '../theme';
import { AgentFolder } from './AgentFolder';
import { AppVersion } from './AppVersion';
import { AutoCompact } from './AutoCompact';
import { HandoffLimit } from './HandoffLimit';
import { StaleDays } from './StaleDays';

/** Settings > General (startup/v1): what Gathr does on this computer, whoever its accounts are. */
export function General({ onReference }: { onReference: (ref: string) => void }) {
  const [theme, setShown] = useState(savedTheme);
  const [on, setOn] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const newest = useNewestWins();
  // A read while a save is out may answer the state before it, so none starts then.
  const pending = useRef(false);

  // Read again on coming back to the window: the person may have changed it in the system's settings.
  useEffect(() => {
    const read = () => {
      if (pending.current) return;
      newest(api.startup()).then((next) => {
        setOn(next);
        if (next) setFailure(null);
      }, setFailure);
    };
    read();
    window.addEventListener('focus', read);
    return () => window.removeEventListener('focus', read);
  }, [newest]);

  /** Shows the change at once; a failure puts the switch back as it was. */
  const change = (next: boolean) => {
    const before = on;
    pending.current = true;
    setOn(next);
    setSaving(true);
    setFailure(null);
    const saved = api.setStartup(next).finally(() => {
      pending.current = false;
      setSaving(false);
    });
    newest(saved).then(setOn, (f: Failure) => {
      setOn(before);
      setFailure(f);
    });
  };

  return (
    <section aria-labelledby="general-title">
      <h3 id="general-title">General</h3>
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
      <div className="settingrow">
        <label htmlFor="theme">Theme</label>
        <span className="setting-scope" id="theme-scope">
          This computer
        </span>
        <select
          id="theme"
          aria-describedby="theme-scope"
          value={theme}
          onChange={(e) => {
            const next = e.target.value as Theme;
            setTheme(next);
            setShown(next);
          }}
        >
          <option value="system">System</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </div>
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
      <AgentFolder onReference={onReference} />
      <StaleDays onReference={onReference} />
      <HandoffLimit onReference={onReference} />
      <AutoCompact onReference={onReference} />
      <AppVersion onReference={onReference} />
    </section>
  );
}
