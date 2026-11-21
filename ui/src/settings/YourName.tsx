import { useState } from 'react';
import { Row } from './Row';
import { useSettings } from './saved';

/** Settings > General: what agents call the person, saved on Enter or on leaving the field. */
export function YourName() {
  const { saved, save } = useSettings();
  // What the person typed, until it is saved.
  const [typed, setTyped] = useState<string | null>(null);
  if (!saved) return null;
  const name = saved.person_name ?? '';
  const keep = () => {
    if (typed !== null && typed.trim() !== name)
      save({ person_name: typed.trim() || null }).then((taken) => taken && setTyped(null));
  };

  return (
    <Row name={<label htmlFor="person-name">Your name</label>} line="What agents call you.">
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
