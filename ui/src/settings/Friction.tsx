import { useEffect, useState } from 'react';
import { api, type Failure, type FrictionReport } from '../api';
import { FailureNotice } from '../components/Notice';

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

/** Settings > Diagnostics: what agents reported got in their way (0048), newest first. */
export function Friction() {
  const [list, setList] = useState<FrictionReport[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  useEffect(() => {
    api.friction().then(setList, setFailure);
  }, []);

  return (
    <section className="friction" aria-labelledby="friction-title">
      <h3 id="friction-title">Friction</h3>
      {failure && <FailureNotice failure={failure} />}
      {list?.length === 0 && <p className="empty">No friction reported yet</p>}
      {list && list.length > 0 && (
        <ul>
          {list.map((f) => (
            <li key={f.id}>
              <p>{f.what}</p>
              <p className="muted">{facts(f)}</p>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
