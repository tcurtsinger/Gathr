import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure } from '../api';
import { Confirm, danger } from '../components/Confirm';
import { useCard } from '../components/Corner';
import { Face, KnownFaces } from '../components/Face';
import { ScheduledMark } from '../components/ScheduledMark';
import { Switch } from '../components/Switch';
import { listen } from '../live';
import '../plugins/plugins.css';
import { Screen } from '../Screen';
import type { Schedule, ScheduleRun, ScheduleShown } from './schedule';
import './scheduled.css';

/** Opens the Scheduled screen at a schedule: its line in a chat calls it. */
export const OpenScheduled = createContext<((id: string) => void) | undefined>(undefined);

interface Props {
  /** The schedule open; none opens the first. */
  at?: string;
  onAt: (id: string) => void;
  /** Opens a chat, at a message of it when one is given. */
  onChat: (chat: string, message?: string) => void;
}

const RELOAD = ['schedules', 'agents', 'chat', 'lagged'];

/**
 * The Scheduled screen, in place of the chat list and the chat: everything scheduled in every chat in its sidebar, a
 * disabled one muted, each by its name; the open one's page with Run now, Delete, Enabled, when it runs, when next,
 * its message and its last runs.
 * Nothing here changes its words or time: the person deletes it and asks an agent again.
 */
export function Scheduled({ at, onAt, onChat }: Props) {
  const [list, setList] = useState<Schedule[]>();
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  const load = useCallback(() => api.schedules().then(setList, setFailure), []);
  useEffect(() => {
    load();
    return listen((e) => {
      if (RELOAD.includes(e.type)) load();
    });
  }, [load]);
  const open = list?.find((s) => s.id === at) ?? list?.[0];
  return (
    <Screen
      name="Scheduled"
      nav={
        <div className="chatlist scheduled-nav">
          {list?.length === 0 && <p className="muted scheduled-none">Nothing scheduled</p>}
          {list?.map((s) => (
            <div
              key={s.id}
              className="chatrow"
              data-current={s.id === open?.id || undefined}
              data-disabled={!s.enabled || undefined}
            >
              <button
                type="button"
                className="chat-link"
                aria-current={s.id === open?.id ? 'page' : undefined}
                onClick={() => onAt(s.id)}
              >
                <Face agent={{ id: s.agent, name: '' }} size={20} />
                <span>{s.name}</span>
              </button>
            </div>
          ))}
        </div>
      }
    >
      {open && <Page key={open.id} schedule={open} onChat={onChat} onFailure={setFailure} />}
    </Screen>
  );
}

interface PageProps {
  /** As the list has it now. */
  schedule: Schedule;
  onChat: Props['onChat'];
  onFailure: (failure: Failure) => void;
}

/** One schedule's page, as the owner's pick draws it, on Plugins' page of a tool. */
function Page({ schedule: s, onChat, onFailure }: PageProps) {
  const [shown, setShown] = useState<ScheduleShown>();
  const [running, setRunning] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const remove = useRef<HTMLButtonElement>(null);
  const agent = useContext(KnownFaces).get(s.agent)?.name ?? 'An agent';
  // Asked again with each change to the list: a run, Enabled, its next time.
  const listed = JSON.stringify(s);
  useEffect(() => {
    if (listed) api.schedule(s.id).then(setShown, onFailure);
  }, [s.id, listed, onFailure]);
  const run = () => {
    setRunning(true);
    api
      .runSchedule(s.id)
      .catch(onFailure)
      .finally(() => setRunning(false));
  };
  return (
    <article className="skill-page">
      <header className="screen-head">
        <span className="plugin-tile" aria-hidden="true">
          <ScheduledMark />
        </span>
        <div className="screen-title">
          <h1>{s.name}</h1>
          <p className="muted scheduled-where">
            {agent} · in{' '}
            <button type="button" className="scheduled-chat" onClick={() => onChat(s.chat)}>
              {s.chat_title}
              {s.project && `, ${s.project}`}
            </button>
          </p>
        </div>
        <div className="screen-actions">
          <Button type="button" size="sm" variant="outline" disabled={running} onClick={run}>
            Run now
          </Button>
          <Button
            ref={remove}
            type="button"
            size="sm"
            variant="outline"
            className={danger}
            onClick={() => setDeleting(true)}
          >
            Delete
          </Button>
        </div>
      </header>
      <div className="skill-enabled">
        <span className="skill-enabled-words">
          <span id="schedule-enabled">Enabled</span>
          <span className="muted" id="schedule-enabled-why">
            When disabled, it doesn’t run.
          </span>
        </span>
        <Switch
          on={s.enabled}
          labelledBy="schedule-enabled"
          describedBy="schedule-enabled-why"
          onChange={(on) => api.enableSchedule(s.id, on).catch(onFailure)}
        />
      </div>
      <div className="scheduled-row">
        <span className="skill-enabled-words">
          <span>When</span>
          <span className="muted">{s.when}</span>
        </span>
      </div>
      {s.next && (
        <div className="scheduled-row">
          <span className="skill-enabled-words">
            <span>Next</span>
            <span className="muted">{dayTime(s.next)}</span>
          </span>
        </div>
      )}
      <div className="scheduled-row">
        <span className="skill-enabled-words">
          <span>Message</span>
          <span className="muted scheduled-words">{s.words}</span>
        </span>
      </div>
      {shown && shown.runs.length > 0 && (
        <section className="tool-actions scheduled-runs">
          <h2>Runs</h2>
          <ul>
            {shown.runs.map((r) => (
              <li key={r.id}>
                <button
                  type="button"
                  className="scheduled-run"
                  disabled={!r.message}
                  onClick={() => r.message && onChat(s.chat, r.message)}
                >
                  <strong>
                    {r.state === 'skipped' ? dayTime(r.at, false) : dayTime(r.at)}
                    <span className="scheduled-state" data-state={r.state}>
                      {STATES[r.state]}
                    </span>
                  </strong>
                  {r.line && <span className="muted">{r.line}</span>}
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
      {deleting && (
        <Confirm
          question={`Delete ${s.name}?`}
          verb="Delete"
          danger
          opener={remove.current}
          onCancel={() => setDeleting(false)}
          onConfirm={() => api.deleteSchedule(s.id)}
        />
      )}
    </article>
  );
}

const STATES: Record<ScheduleRun['state'], string> = {
  running: 'Running',
  done: 'Done',
  late: 'Ran late',
  skipped: 'Skipped',
  failed: 'Failed',
  stopped: 'Stopped',
};

/** "Today, 9:00", "Tomorrow, 9:00", "Friday, 9:00", "23 Nov, 9:00": on the 24-hour clock the screen's "When" uses. */
export function dayTime(iso: string, time = true) {
  const at = new Date(iso);
  const day = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((day(at) - day(new Date())) / 86_400_000);
  const named =
    days === 0
      ? 'Today'
      : days === 1
        ? 'Tomorrow'
        : days === -1
          ? 'Yesterday'
          : Math.abs(days) < 7
            ? at.toLocaleDateString([], { weekday: 'long' })
            : at.toLocaleDateString([], { day: 'numeric', month: 'short' });
  if (!time) return named;
  return `${named}, ${at.getHours()}:${String(at.getMinutes()).padStart(2, '0')}`;
}
