import { useState } from 'react';
import { FailureNotice } from '../components/Notice';
import { Row } from './Row';
import { useSettings } from './saved';

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
