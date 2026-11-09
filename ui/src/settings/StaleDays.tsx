import { useEffect, useState } from 'react';
import { api, type StaleDays as Days, type Failure } from '../api';
import { Help } from '../components/Help';
import { Notice } from '../components/Notice';
import { useNewestWins } from '../newest';

const rows = [
  {
    key: 'stale_days',
    label: 'Stale after',
    unit: 'days',
    help: 'Days without progress. Turns, messages, answers and fired waits reset the timer.',
  },
  {
    key: 'cancel_days',
    label: 'Cancel after',
    unit: 'days stale',
    help: 'Days stale before auto-cancel. Active work keeps agent tasks open. Your questions never auto-cancel.',
  },
] as const;

/** Settings > General's stale rules (F11, archive-stale/v1): whole days, saved on a committed edit. */
export function StaleDays({ onReference }: { onReference: (ref: string) => void }) {
  const [saved, setSaved] = useState<Days | null>(null);
  // What the person typed, kept as typed until it saves: an invalid or refused edit stays visible.
  const [edits, setEdits] = useState<Partial<Record<keyof Days, string>>>({});
  const [invalid, setInvalid] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const newest = useNewestWins();

  useEffect(() => {
    api.settings().then(setSaved, setFailure);
  }, []);

  const commit = (key: keyof Days) => {
    const typed = edits[key];
    if (typed === undefined || !saved) return;
    const days = Number(typed);
    if (!/^\d+$/.test(typed.trim()) || days < 1 || days > 365) return setInvalid(true);
    setInvalid(false);
    setFailure(null);
    // Saved even when it equals what shows: a save still out may be about to change that. Only the
    // newest request's answer shows, and it clears the edit only if that's still what was saved.
    newest(api.saveSettings({ [key]: days })).then(
      (next) => {
        setSaved(next);
        setEdits((all) => {
          if (all[key] !== typed) return all;
          const { [key]: _, ...rest } = all;
          return rest;
        });
      },
      (f: Failure) => {
        setFailure(f);
        newest(api.settings()).then(setSaved, setFailure);
      },
    );
  };

  return (
    <>
      {failure && (
        <Notice
          danger
          title={failure.error}
          reference={failure.ref ? { ref: failure.ref, open: onReference } : undefined}
          links={failure.why && <span className="muted">{failure.why}</span>}
        />
      )}
      {saved &&
        rows.map((r) => (
          <div className="settingrow" key={r.key}>
            <span className="setting-label">
              <label htmlFor={r.key}>{r.label}</label>
              <Help label={`About ${r.label.toLowerCase()}`}>
                <p>{r.help}</p>
              </Help>
            </span>
            <span className="setting-days">
              <input
                id={r.key}
                type="number"
                min={1}
                max={365}
                step={1}
                inputMode="numeric"
                aria-describedby={`${r.key}-unit`}
                aria-invalid={invalid && edits[r.key] !== undefined}
                value={edits[r.key] ?? saved[r.key]}
                onChange={(e) => setEdits((all) => ({ ...all, [r.key]: e.target.value }))}
                onBlur={() => commit(r.key)}
                onKeyDown={(e) => e.key === 'Enter' && commit(r.key)}
              />
              <small id={`${r.key}-unit`}>{r.unit}</small>
            </span>
          </div>
        ))}
      {invalid && (
        <p className="setting-invalid" role="alert">
          Use whole days: 1 to 365.
        </p>
      )}
    </>
  );
}
