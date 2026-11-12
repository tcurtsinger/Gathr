import { type ReactNode, useEffect, useState } from 'react';
import { api, type Failure, type Settings } from '../api';
import { Help } from '../components/Help';
import { FailureNotice } from '../components/Notice';
import { useNewestWins } from '../newest';

const NONE = 'none';

interface Props {
  /** The setting it shows and saves: a number, or null. */
  field: 'handoff_limit' | 'auto_compact';
  label: string;
  help: { label: string; text: ReactNode };
  /** Each choice, null for the setting's none. */
  choices: { value: number | null; label: string }[];
  /** A value set another way (through the API) that isn't one of the choices, as it reads. */
  other: (value: number) => string;
  onReference: (ref: string) => void;
}

/**
 * A setting in Settings > General picked from a list, saved when picked; a refused pick says why and shows
 * what's saved.
 */
export function SettingChoice({ field, label, help, choices, other, onReference }: Props) {
  const [saved, setSaved] = useState<number | null | undefined>(undefined);
  const [failure, setFailure] = useState<Failure | null>(null);
  const newest = useNewestWins();
  const show = (s: Settings) => setSaved(s[field]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: read once, as the page opens
  useEffect(() => {
    api.settings().then(show, setFailure);
  }, []);

  const pick = (value: string) => {
    const next = value === NONE ? null : Number(value);
    setSaved(next);
    setFailure(null);
    newest(api.saveSettings({ [field]: next })).then(show, (f: Failure) => {
      setFailure(f);
      newest(api.settings()).then(show, setFailure);
    });
  };

  return (
    <>
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
      {saved !== undefined && (
        <div className="settingrow">
          <span className="setting-label">
            <label htmlFor={field}>{label}</label>
            <Help label={help.label}>{help.text}</Help>
          </span>
          <select id={field} value={saved ?? NONE} onChange={(e) => pick(e.target.value)}>
            {saved !== null && !choices.some((c) => c.value === saved) && (
              <option value={saved}>{other(saved)}</option>
            )}
            {choices.map((c) => (
              <option key={c.value ?? NONE} value={c.value ?? NONE}>
                {c.label}
              </option>
            ))}
          </select>
        </div>
      )}
    </>
  );
}
