import { useEffect, useMemo, useState } from 'react';
import { api, type Failure, type TurnTimeline } from '../api';
import { Notice } from '../components/Notice';
import { facts, said, seconds, steps } from './timeline';
import './diagnostics.css';

const filters = ['All', 'Failed', 'Stopped', 'Completed'] as const;
type Filter = (typeof filters)[number];

interface Props {
  chat?: string;
  title?: string;
  /** Opens at this turn. */
  turn?: string;
}

/** Settings > Diagnostics: a chat's turns beside the chosen turn's timeline (diagnostics/v3). */
export function Diagnostics({ chat, title, turn }: Props) {
  const [list, setList] = useState<TurnTimeline[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [filter, setFilter] = useState<Filter>('All');
  const [find, setFind] = useState('');
  const [picked, setPicked] = useState<string | undefined>(turn);

  useEffect(() => {
    if (!chat) return;
    api.turns(chat).then((t) => {
      setList(t);
      setPicked((p) => p ?? t.at(-1)?.id);
    }, setFailure);
  }, [chat]);

  // Opening at a turn shows it, whatever the filters were before (DESIGN.md).
  useEffect(() => {
    if (!turn) return;
    setPicked(turn);
    setFilter('All');
    setFind('');
  }, [turn]);

  const shown = useMemo(() => {
    const q = find.trim().toLowerCase();
    return (list ?? [])
      .filter((t) => filter === 'All' || said(t.status) === filter)
      .filter((t) => !q || t.ref.startsWith(q) || String(t.n) === q || t.asked?.toLowerCase().includes(q))
      .reverse();
  }, [list, filter, find]);
  const current = list?.find((t) => t.id === picked);

  if (!chat) return <p className="empty">No chat open</p>;
  return (
    <section className="diagnostics" aria-labelledby="diagnostics-title">
      <div className="diagnostics-head">
        <h3 id="diagnostics-title">{title ?? 'Diagnostics'}</h3>
        <div className="toolbar">
          <label>
            Turns{' '}
            <select value={filter} onChange={(e) => setFilter(e.target.value as Filter)}>
              {filters.map((f) => (
                <option key={f}>{f}</option>
              ))}
            </select>
          </label>
          <label>
            Find{' '}
            <input value={find} placeholder="Turn or reference" onChange={(e) => setFind(e.target.value)} />
          </label>
          {list && <span className="muted">{list.length === 1 ? '1 turn' : `${list.length} turns`}</span>}
        </div>
      </div>
      {failure && <Notice danger title={failure.error} />}
      {list?.length === 0 && <p className="empty">No turns yet</p>}
      {list && list.length > 0 && (
        <div className="split">
          <div className="turnlist">
            {shown.map((t) => (
              <button
                type="button"
                key={t.id}
                className="turnpick"
                aria-pressed={t.id === picked}
                onClick={() => setPicked(t.id)}
              >
                Turn {t.n}: {said(t.status)}
                <span>{t.asked}</span>
                <span>{facts(t).summary}</span>
              </button>
            ))}
          </div>
          {current && <Timeline turn={current} />}
        </div>
      )}
    </section>
  );
}

function Timeline({ turn }: { turn: TurnTimeline }) {
  const f = facts(turn);
  return (
    <div className="trace">
      <h4>Turn {turn.n}</h4>
      <dl className="facts">
        <div>
          <dt>Ended</dt>
          <dd>{said(turn.status)}</dd>
        </div>
        {f.duration !== null && (
          <div>
            <dt>Duration</dt>
            <dd>{seconds(f.duration)}</dd>
          </div>
        )}
        {f.tokens.map(([label, n]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{n.toLocaleString()}</dd>
          </div>
        ))}
      </dl>
      {turn.status === 'failed' && (
        <>
          <Notice danger title="Turn failed" links={<code className="ref">{turn.ref}</code>} />
          {turn.why && <p>{turn.why}</p>}
        </>
      )}
      <ol className="events">
        {steps(turn.events).map((step) =>
          step.kind === 'tools' ? (
            <li key={step.key}>
              <details>
                <summary>{step.events.length} tool calls</summary>
                <ol className="events">
                  {step.events.map((e) => (
                    <Event key={e.key} label={e.label} at={e.at} duration={e.duration} failed={e.failed} />
                  ))}
                </ol>
              </details>
            </li>
          ) : (
            <Event
              key={step.key}
              label={step.label}
              at={step.at}
              duration={step.duration}
              failed={step.failed}
            />
          ),
        )}
      </ol>
    </div>
  );
}

function Event(e: { label: string; at: number; duration: number | null; failed: boolean }) {
  return (
    <li className={e.failed ? 'event failed' : 'event'}>
      <time>+{seconds(e.at)}</time>
      <span>{e.label}</span>
      <small>{e.duration === null ? '' : seconds(e.duration)}</small>
    </li>
  );
}
