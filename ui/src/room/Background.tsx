import { ChevronRight, Square, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import { api, type Background, type Failure } from '../api';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { IconButton } from '../components/IconButton';
import { since } from '../components/marks';
import './background.css';

/** The Background tab's mark: two diamonds, one over the other's edge, on the icon set's grid. */
export function BackgroundTasksMark() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M9 5.5 2.5 12 9 18.5 15.5 12Z" />
      <path d="m12.5 8.5 3.5-3.5 6.5 7-6.5 7-3.5-3.5" />
    </svg>
  );
}

/** The commands still running, oldest first. */
export const runningOf = (tasks: Background[]) => tasks.filter((t) => !t.ended);

/** Home's Background tile: how many run now. */
export function BackgroundTile({ tasks, onOpen }: { tasks: Background[]; onOpen: () => void }) {
  const running = runningOf(tasks).length;
  return (
    <button type="button" className="home-tile" onClick={onOpen}>
      <span className="home-tile-name">
        <BackgroundTasksMark />
        Background
      </span>
      <span className="home-tile-sub">{running ? `${running} running` : 'None'}</span>
    </button>
  );
}

/**
 * The Background tab: what the chat's agents run in the background now, each with how long and Stop, then what
 * ended, newest first, with how long it took or how it failed. A row opens to the end of what it wrote.
 */
export function BackgroundView(props: {
  chat: string;
  tasks: Background[];
  /** The agent's name, for its face: none when the chat has one agent. */
  face?: (agent: string) => string;
}) {
  const { chat, tasks } = props;
  const [folded, setFolded] = useState(false);
  const [failure, setFailure] = useState<Told>();
  useCard(failure);
  const running = runningOf(tasks);
  const ended = tasks.filter((t) => t.ended).reverse();
  const now = useNow(running.length > 0);
  if (!tasks.length)
    return (
      <div className="filetab-none">
        <BackgroundTasksMark />
        <strong>No background tasks</strong>
        <span>Commands and helpers agents leave running show here.</span>
      </div>
    );
  const row = (t: Background) => (
    <Row key={t.id} chat={chat} task={t} now={now} face={props.face} onFailure={setFailure} />
  );
  return (
    <div className="bg">
      <div className="bg-scroll">
        {running.length > 0 && (
          <section>
            <h3 className="bg-group">
              Running <span>{running.length}</span>
            </h3>
            {running.map(row)}
          </section>
        )}
        {ended.length > 0 && (
          <section>
            <h3 className="bg-group">
              <button type="button" aria-expanded={!folded} onClick={() => setFolded(!folded)}>
                <ChevronRight aria-hidden="true" className="bg-chev" />
                Finished <span>{ended.length}</span>
              </button>
              <span className="room-grow" />
              <IconButton
                aria-label="Clear finished"
                title="Clear finished"
                onClick={() =>
                  api.clearBackground(chat).catch((f: Failure) => setFailure(told('Clear failed', f)))
                }
              >
                <Trash2 className="bg-mark" />
              </IconButton>
            </h3>
            {!folded && ended.map(row)}
          </section>
        )}
      </div>
    </div>
  );
}

/** One command: a click opens what it wrote, once the engine said where. */
function Row(props: {
  chat: string;
  task: Background;
  now: number;
  face?: (agent: string) => string;
  onFailure: (f: Told) => void;
}) {
  const { chat, task: t, now } = props;
  const [open, setOpen] = useState(false);
  const [output, setOutput] = useState<{ text: string | null; cut: boolean }>();
  const [stopping, setStopping] = useState(false);
  const can = t.output;
  useEffect(() => {
    if (!open || !can) return;
    let live = true;
    api.backgroundOutput(chat, t.id).then(
      (o) => live && setOutput(o),
      () => live && setOutput({ text: null, cut: false }),
    );
    return () => {
      live = false;
    };
  }, [open, can, chat, t.id]);
  const took = t.ended ? Date.parse(t.ended) - Date.parse(t.started) : now - Date.parse(t.started);
  const stop = () => {
    setStopping(true);
    api.stopBackground(chat, t.id).catch((f: Failure) => {
      setStopping(false);
      props.onFailure(told('Stop failed', f));
    });
  };
  const name = t.agent && props.face?.(t.agent);
  return (
    <div className="bg-task" data-open={open || undefined}>
      <div className="bg-head">
        <button
          type="button"
          className="bg-what"
          aria-expanded={can ? open : undefined}
          disabled={!can}
          onClick={() => setOpen(!open)}
        >
          <ChevronRight aria-hidden="true" className="bg-chev" data-hidden={!can || undefined} />
          {name && t.agent && <Face agent={{ id: t.agent, name }} size={16} />}
          <span className="bg-name" data-cmd={t.kind === 'local_bash' || undefined} title={t.description}>
            {t.description}
          </span>
        </button>
        {t.status === 'failed' ? (
          <span className="bg-end" data-failed="">
            Failed
          </span>
        ) : t.status === 'stopped' ? (
          <span className="bg-end">Stopped</span>
        ) : (
          <span className="bg-end">{since(took, true)}</span>
        )}
        {!t.ended && (
          <IconButton aria-label="Stop" title="Stop" className="bg-stop" disabled={stopping} onClick={stop}>
            <Square className="bg-stop-mark" />
          </IconButton>
        )}
      </div>
      {open && output && (
        <pre className="bg-out">
          {output.text === null ? (
            <span className="bg-gone">Output is gone</span>
          ) : (
            <>
              {output.cut && <span className="bg-gone">Earlier output cut{'\n'}</span>}
              {output.text.trimEnd() || <span className="bg-gone">No output</span>}
            </>
          )}
        </pre>
      )}
    </div>
  );
}

type Told = Pick<Failure, 'error' | 'why' | 'ref'>;
const told = (error: string, f: Failure): Told => ({ error, why: f.why, ref: f.ref });

/** Now, read again each second while anything runs. */
function useNow(ticking: boolean) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!ticking) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [ticking]);
  return ticking ? Math.max(now, Date.now()) : now;
}
