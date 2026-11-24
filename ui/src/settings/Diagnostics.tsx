import { ChevronRight } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { api, type Chat, type Failure, type Named, type TurnEvent, type TurnTimeline } from '../api';
import { useCard } from '../components/Corner';
import { Dropdown } from '../components/Dropdown';
import { Face } from '../components/Face';
import { Page } from './Row';
import { facts, said, seconds, steps } from './timeline';
import './diagnostics.css';

const filters = ['All', 'Failed', 'Stopped', 'Completed'] as const;
type Filter = (typeof filters)[number];

interface Props {
  chat?: string;
  /** Opens at this turn. */
  turn?: string;
  /** Shows this reference's events instead, for one with no turn behind it (a sign-in). */
  reference?: string;
}

/** Settings > Turns: a chat's turns beside the chosen turn's timeline (diagnostics/v3). */
export function Diagnostics({ reference, ...turns }: Props) {
  return reference ? <Reference reference={reference} /> : <Turns {...turns} />;
}

function Reference({ reference }: { reference: string }) {
  const [events, setEvents] = useState<TurnEvent[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  useEffect(() => {
    setEvents(null);
    setFailure(null);
    api.trace(reference).then(setEvents, setFailure);
  }, [reference]);
  // A refused request's event says why under `reason`, as `gathr diag trace` shows it.
  const why = events
    ?.filter((e) => e.status === 'error')
    .map((e) => e.attrs?.why ?? e.attrs?.reason)
    .findLast((w) => typeof w === 'string');

  return (
    <Page wide title={`Reference ${reference}`}>
      {events && (
        <div className="trace">
          {typeof why === 'string' && <p>{why}</p>}
          <Steps events={events} />
        </div>
      )}
    </Page>
  );
}

function Turns({ chat: open, turn }: Omit<Props, 'reference'>) {
  const [chats, setChats] = useState<Chat[]>([]);
  // The chat Settings opened over, or another picked here.
  const [chat, setChat] = useState(open);
  const [agents, setAgents] = useState<Named[]>([]);
  const [list, setList] = useState<TurnTimeline[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  const [filter, setFilter] = useState<Filter>('All');
  const [find, setFind] = useState('');
  const [picked, setPicked] = useState<string | undefined>(turn);

  useEffect(() => {
    api.chats().then((c) => {
      setChats(c);
      setChat((now) => now ?? c[0]?.id);
    }, setFailure);
    api.agents().then((a) => setAgents([...a.agents, ...a.removed]), setFailure);
  }, []);

  useEffect(() => {
    if (!chat) return;
    setList(null);
    api.turns(chat).then((t) => {
      setList(t);
      setPicked((p) => (p && t.some((x) => x.id === p) ? p : t.at(-1)?.id));
    }, setFailure);
  }, [chat]);

  // Opening at a turn shows it, whatever the filters were before (DESIGN.md).
  useEffect(() => {
    if (!turn) return;
    setChat(open);
    setPicked(turn);
    setFilter('All');
    setFind('');
  }, [turn, open]);

  const shown = useMemo(() => {
    const q = find.trim().toLowerCase();
    return (list ?? [])
      .filter((t) => filter === 'All' || said(t.status) === filter)
      .filter((t) => !q || t.ref.startsWith(q) || String(t.n) === q || t.asked?.toLowerCase().includes(q))
      .reverse();
  }, [list, filter, find]);
  // A turn the filters hide isn't shown beside a list it isn't in.
  const current = shown.find((t) => t.id === picked);
  const who = (id: string | null) => agents.find((a) => a.id === id);
  const count = list
    ? shown.length === list.length
      ? `${list.length} turn${list.length === 1 ? '' : 's'}`
      : `${shown.length} of ${list.length} turns`
    : '';

  return (
    <Page wide title="Turns" line="Each turn, step by step.">
      <div className="toolbar">
        <label htmlFor="diagnostics-chat">Chat</label>
        <Dropdown
          id="diagnostics-chat"
          className="turns-chat"
          value={chat ?? ''}
          placeholder="No chats"
          onChange={(c) => {
            setChat(c);
            setPicked(undefined);
          }}
          choices={chats.map((c) => ({ value: c.id, label: c.title }))}
        />
        <label htmlFor="diagnostics-filter">Show</label>
        <Dropdown
          id="diagnostics-filter"
          value={filter}
          onChange={(v) => setFilter(v as Filter)}
          choices={filters.map((f) => ({ value: f, label: f }))}
        />
        <label>
          Find{' '}
          <input value={find} placeholder="Turn or reference" onChange={(e) => setFind(e.target.value)} />
        </label>
        <span className="muted">{count}</span>
      </div>
      {list?.length === 0 && <p className="settings-empty">No turns yet</p>}
      {list && list.length > 0 && (
        <div className="split">
          <div className="turnlist">
            {shown.length === 0 && <p className="settings-empty">No turns match</p>}
            {shown.map((t) => {
              const agent = who(t.agent);
              return (
                <button
                  type="button"
                  key={t.id}
                  className="turnpick"
                  aria-pressed={t.id === picked}
                  onClick={() => setPicked(t.id)}
                >
                  {agent && <Face agent={agent} size={16} />}
                  <span className="turnpick-words">
                    <span className="turnpick-head">
                      Turn {t.n}
                      {agent && ` · ${agent.name}`} · <b data-status={t.status}>{said(t.status)}</b>
                    </span>
                    {t.asked && <span>{t.asked}</span>}
                  </span>
                </button>
              );
            })}
          </div>
          {current && <Timeline turn={current} agent={who(current.agent)} />}
        </div>
      )}
    </Page>
  );
}

function Timeline({ turn, agent }: { turn: TurnTimeline; agent?: Named }) {
  const f = facts(turn);
  return (
    <div className="trace">
      <h4>
        {agent && <Face agent={agent} size={16} />}
        Turn {turn.n}
        {agent && ` · ${agent.name}`}
      </h4>
      {turn.status === 'failed' && turn.why && <p className="turn-why">{turn.why}</p>}
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
        {f.given.map(([label, size]) => (
          <div key={label} className="two">
            <dt>{label}</dt>
            <dd>{size}</dd>
          </div>
        ))}
        {turn.status === 'failed' && (
          <div>
            <dt>Reference</dt>
            <dd>
              <code>{turn.ref}</code>
            </dd>
          </div>
        )}
      </dl>
      <Steps events={turn.events} />
    </div>
  );
}

function Steps({ events }: { events: TurnEvent[] }) {
  return (
    <ol className="events">
      {steps(events).map((step) =>
        step.kind === 'tools' ? (
          <li key={step.key}>
            <details>
              <summary>
                <ChevronRight aria-hidden="true" />
                {step.events.length} tool calls
              </summary>
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
