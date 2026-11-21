import { useEffect, useState } from 'react';
import { api, type Failure, type FrictionReport } from '../api';
import { useCard } from '../components/Corner';
import { Page, Row } from './Row';

const when = (time: string) =>
  new Date(time).toLocaleString([], {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  });

/** Who reported it, on which task, in which chat, when and where: what there is of each. */
const facts = (f: FrictionReport) =>
  [f.agent_name ?? 'An agent', f.task ? `Task #${f.task}` : null, f.chat_title, when(f.time), f.where]
    .filter(Boolean)
    .join(' · ');

/** Settings > Friction: what agents reported got in their way (0048), newest first. */
export function Friction() {
  const [list, setList] = useState<FrictionReport[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  useEffect(() => {
    api.friction().then(setList, setFailure);
  }, []);

  return (
    <Page title="Friction" line="What agents reported got in their way, newest first.">
      {list?.length === 0 && <p className="settings-empty">No friction reported yet</p>}
      {list && list.length > 0 && (
        <ul className="settings-rows settings-top friction">
          {list.map((f) => (
            <li key={f.id}>
              <Row name={f.what} line={facts(f)} />
            </li>
          ))}
        </ul>
      )}
    </Page>
  );
}
