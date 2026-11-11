import { useEffect, useState } from 'react';
import { api, type Failure } from '../api';
import { Help } from '../components/Help';
import { FailureNotice } from '../components/Notice';
import { useNewestWins } from '../newest';

/** The owner's list (room task 41): no Off, which would bring back what the limit fixes. */
const limits = [5, 10, 25, 50];
const NONE = 'none';

/**
 * Settings > General: how many messages between agents in a row a team chat takes before it pauses
 * (plans/agent-messages.md). Saved when picked; a refused pick says why and shows what's saved.
 */
export function HandoffLimit({ onReference }: { onReference: (ref: string) => void }) {
  const [saved, setSaved] = useState<number | null | undefined>(undefined);
  const [failure, setFailure] = useState<Failure | null>(null);
  const newest = useNewestWins();

  useEffect(() => {
    api.settings().then((s) => setSaved(s.handoff_limit), setFailure);
  }, []);

  const pick = (value: string) => {
    const limit = value === NONE ? null : Number(value);
    setSaved(limit);
    setFailure(null);
    newest(api.saveSettings({ handoff_limit: limit })).then(
      (s) => setSaved(s.handoff_limit),
      (f: Failure) => {
        setFailure(f);
        newest(api.settings()).then((s) => setSaved(s.handoff_limit), setFailure);
      },
    );
  };

  return (
    <>
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
      {saved !== undefined && (
        <div className="settingrow">
          <span className="setting-label">
            <label htmlFor="handoff-limit">Pause after</label>
            <Help label="About hand-offs">
              <p>
                Messages between agents in a row, with nothing from you and no task finished between them.
                Past the limit the chat pauses until you resume it.
              </p>
            </Help>
          </span>
          <select id="handoff-limit" value={saved ?? NONE} onChange={(e) => pick(e.target.value)}>
            {/* A limit set another way (the API takes 1 to 1000) still shows as it is. */}
            {saved !== null && !limits.includes(saved) && <option value={saved}>{saved} hand-offs</option>}
            {limits.map((n) => (
              <option key={n} value={n}>
                {n} hand-offs{n === 50 ? ' (default)' : ''}
              </option>
            ))}
            <option value={NONE}>No limit</option>
          </select>
        </div>
      )}
    </>
  );
}
