import { useEffect, useState } from 'react';
import { type Agent, api, type Failure, type Settings } from '../api';
import { Dropdown } from '../components/Dropdown';
import { Face } from '../components/Face';
import { FailureNotice } from '../components/Notice';
import { useNewestWins } from '../newest';

const NONE = 'none';

/**
 * Settings > General: what agents call the person, saved on Enter or on leaving the field, and the assistant a new
 * chat's message goes to, saved when picked. A refusal says why and shows what's saved.
 */
export function Assistant({ onReference }: { onReference: (ref: string) => void }) {
  const [saved, setSaved] = useState<Settings | null>(null);
  const [agents, setAgents] = useState<Agent[]>([]);
  // What the person typed in Your name, until it is saved.
  const [typed, setTyped] = useState<string | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const newest = useNewestWins();

  useEffect(() => {
    api.settings().then(setSaved, setFailure);
    api.agents().then((a) => setAgents(a.agents), setFailure);
  }, []);

  const save = (change: Partial<Settings>) => {
    setFailure(null);
    newest(api.saveSettings(change)).then(
      (s) => {
        setSaved(s);
        setTyped(null);
      },
      (f: Failure) => {
        setFailure(f);
        newest(api.settings()).then(setSaved, setFailure);
      },
    );
  };
  if (!saved) return failure && <FailureNotice failure={failure} onReference={onReference} />;
  const name = saved.person_name ?? '';
  const keep = () => {
    if (typed !== null && typed.trim() !== name) save({ person_name: typed.trim() || null });
  };

  return (
    <>
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
      <div className="settingrow">
        <label htmlFor="person-name">Your name</label>
        <input
          id="person-name"
          value={typed ?? name}
          maxLength={50}
          autoComplete="off"
          spellCheck={false}
          onChange={(e) => setTyped(e.target.value)}
          onBlur={keep}
          onKeyDown={(e) => e.key === 'Enter' && keep()}
        />
      </div>
      <div className="settingrow">
        <label htmlFor="assistant">Assistant</label>
        <Dropdown
          id="assistant"
          value={saved.assistant ?? NONE}
          onChange={(v) => {
            const assistant = v === NONE ? null : v;
            setSaved({ ...saved, assistant });
            save({ assistant });
          }}
          choices={[
            ...agents.map((a) => ({ value: a.id, label: a.name, icon: <Face agent={a} size={20} /> })),
            { value: NONE, label: 'None' },
          ]}
        />
      </div>
    </>
  );
}
