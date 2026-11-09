import { useEffect, useRef, useState } from 'react';
import { api, type StaleDays as Days, type Failure } from '../api';
import { Help } from '../components/Help';
import { Notice } from '../components/Notice';

const rows = [
  {
    key: 'stale_days',
    label: 'Stale after',
    unit: 'days',
    help: 'Tasks are marked stale after this many days without progress. Turns, messages, answers and fired waits count as progress.',
  },
  {
    key: 'cancel_days',
    label: 'Cancel after',
    unit: 'days stale',
    help: "Agents' tasks are cancelled this many days after they become stale. Open waits, reviews, CI and running commands keep them open. Your questions stay open until you answer or cancel them.",
  },
] as const;

/** Settings > General's stale rules (F11, archive-stale/v1): whole days, saved on a committed edit. */
export function StaleDays({ onReference }: { onReference: (ref: string) => void }) {
  const [saved, setSaved] = useState<Days | null>(null);
  // What the person typed, kept as typed until it saves: an invalid or refused edit stays visible.
  const [edits, setEdits] = useState<Partial<Record<keyof Days, string>>>({});
  const [invalid, setInvalid] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const saves = useRef(0);

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
    if (days === saved[key]) return setEdits(({ [key]: _, ...rest }) => rest);
    // Only the latest save's answer is shown, and it clears the edit only if that's still what was saved.
    const n = ++saves.current;
    api.saveSettings({ [key]: days }).then(
      (next) => {
        if (n !== saves.current) return;
        setSaved(next);
        setEdits((all) => {
          if (all[key] !== typed) return all;
          const { [key]: _, ...rest } = all;
          return rest;
        });
      },
      (f: Failure) => {
        if (n !== saves.current) return;
        setFailure(f);
        api.settings().then(setSaved, setFailure);
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
          Enter whole days from 1 to 365.
        </p>
      )}
    </>
  );
}
