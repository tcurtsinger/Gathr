import { useEffect, useRef, useState } from 'react';
import { api, type Failure } from '../api';
import { Dropdown } from '../components/Dropdown';
import { FailureNotice } from '../components/Notice';
import { Switch } from '../components/Switch';
import { useNewestWins } from '../newest';
import { savedTheme, setTheme, type Theme } from '../theme';
import { AgentFolder } from './AgentFolder';
import { AppVersion } from './AppVersion';
import { Assistant, YourName } from './Assistant';
import { AutoCompact } from './AutoCompact';
import { HandoffLimit } from './HandoffLimit';
import { Page, Row } from './Row';

type Open = (ref: string) => void;

/** Settings > General (startup/v1): the person's name, what Gathr does on this computer, and its version. */
export function General({ onReference }: { onReference: Open }) {
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
    <Page title="General" line="Your name, the look of Gathr and its version.">
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
      <div className="settings-rows">
        <YourName onReference={onReference} />
        <Row
          name={<label htmlFor="theme">Theme</label>}
          line="Applies on this computer only."
          lineId="theme-line"
        >
          <Dropdown
            id="theme"
            aria-describedby="theme-line"
            value={theme}
            onChange={(v) => {
              const next = v as Theme;
              setTheme(next);
              setShown(next);
            }}
            choices={[
              { value: 'system', label: 'System' },
              { value: 'light', label: 'Light' },
              { value: 'dark', label: 'Dark' },
            ]}
          />
        </Row>
        {on !== null && (
          <Row
            name={<span id="startup-label">Open at login</span>}
            line="Opens Gathr when you sign in to this computer."
            lineId="startup-line"
          >
            <span className="visually-hidden" role="status">
              {saving ? 'Saving' : ''}
            </span>
            <Switch
              on={on}
              labelledBy="startup-label"
              describedBy="startup-line"
              disabled={saving}
              onChange={change}
            />
          </Row>
        )}
        <AppVersion onReference={onReference} />
      </div>
    </Page>
  );
}

/** Settings > Teamwork: how agents take a new chat and work together. */
export function Teamwork({ onReference }: { onReference: Open }) {
  return (
    <Page title="Teamwork" line="How your agents take a new chat and work together.">
      <div className="settings-rows">
        <Assistant onReference={onReference} />
        <HandoffLimit onReference={onReference} />
        <AutoCompact onReference={onReference} />
        <AgentFolder onReference={onReference} />
      </div>
    </Page>
  );
}
