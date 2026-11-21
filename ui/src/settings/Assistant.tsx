import { useEffect, useState } from 'react';
import { type Agent, api, type Failure } from '../api';
import { Dropdown } from '../components/Dropdown';
import { Face } from '../components/Face';
import { FailureNotice } from '../components/Notice';
import { Row } from './Row';
import { useSettings } from './saved';

const NONE = 'none';
type Open = (ref: string) => void;

/** Settings > General: what agents call the person, saved on Enter or on leaving the field. */
export function YourName({ onReference }: { onReference: Open }) {
  const { saved, failure, save } = useSettings();
  // What the person typed, until it is saved.
  const [typed, setTyped] = useState<string | null>(null);
  const notice = failure && <FailureNotice failure={failure} onReference={onReference} />;
  if (!saved) return notice;
  const name = saved.person_name ?? '';
  const keep = () => {
    if (typed !== null && typed.trim() !== name)
      save({ person_name: typed.trim() || null }).then((taken) => taken && setTyped(null));
  };

  return (
    <Row name={<label htmlFor="person-name">Your name</label>} line="What agents call you." after={notice}>
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
    </Row>
  );
}

/** Settings > Teamwork: the agent a new chat's message goes to, saved when picked. */
export function Assistant({ onReference }: { onReference: Open }) {
  const { saved, failure, save } = useSettings();
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loadFailure, setLoadFailure] = useState<Failure | null>(null);
  useEffect(() => {
    api.agents().then((a) => setAgents(a.agents), setLoadFailure);
  }, []);
  const shown = failure ?? loadFailure;
  const notice = shown && <FailureNotice failure={shown} onReference={onReference} />;
  if (!saved) return notice;

  return (
    <Row
      name={<label htmlFor="assistant">Assistant</label>}
      line="A new chat’s message goes to this agent."
      after={notice}
    >
      <Dropdown
        id="assistant"
        value={saved.assistant ?? NONE}
        onChange={(v) => save({ assistant: v === NONE ? null : v })}
        choices={[
          ...agents.map((a) => ({ value: a.id, label: a.name, icon: <Face agent={a} size={20} /> })),
          { value: NONE, label: 'None' },
        ]}
      />
    </Row>
  );
}
