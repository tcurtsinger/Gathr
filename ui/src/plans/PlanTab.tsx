import { Fragment, useEffect, useState } from 'react';
import { ElicitationForm } from '@/components/assistant-ui/elements/elicitation-form';
import { Button } from '@/components/ui/button';
import { api, type Failure, type Plan, type PlanPart, type PlanVersion, type TaskField } from '../api';
import { began } from '../chat/team';
import { FailureNotice } from '../components/Notice';
import { listen } from '../live';
import { briefRows, noteLine } from '../tasks/model';
import { counted, marks, type PartField, type PartMark } from './plans';
import '../tasks/tasks.css';
import './plans.css';

interface Props {
  chat: string;
  plan: number;
  /** The version shown; the newest when the plan has no such version yet. */
  version: number;
  name: (agent: string | null) => string;
  onVersion: (version: number) => void;
  onReference: (ref: string) => void;
  /** Whether the plan waits on the person: the tab's dot. */
  onWaits?: (waits: boolean) => void;
}

/** The newest version's state word; waiting on the person has none. */
const word: Record<Plan['state'], string | undefined> = {
  checking: 'Checking',
  waiting: undefined,
  revising: 'Revising',
  approved: 'Approved',
  dropped: 'Dropped',
};

/** The files a catch names, as Gathr's line in the chat does; the rest are counted. */
const NAMED = 20;

/**
 * The lead's plan in its own tab (0062 H5.4, the plan-card mock-up v2): a version, its check with the findings
 * and answers, the ask's questions, and its parts, one open at a time, marked against the version before; with
 * Approve, Send back and Drop at its foot. Read again whenever the chat's plans change.
 */
export function PlanTab({ chat, plan: id, version, name, onVersion, onReference, onWaits }: Props) {
  const [plan, setPlan] = useState<Plan | null>(null);
  const [loadFailure, setLoadFailure] = useState<Failure | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [busy, setBusy] = useState(false);
  // Send back's words, while its field is open.
  const [words, setWords] = useState<string | null>(null);
  const [asked, setAsked] = useState(0);
  // The one part open to its brief, by its version and number: another version opens with none open.
  const [opened, setOpened] = useState<string | null>(null);
  const waiting = plan?.state === 'waiting';
  // biome-ignore lint/correctness/useExhaustiveDependencies: the app's handler is new with each draw; only the plan moves the dot
  useEffect(() => {
    onWaits?.(waiting);
  }, [waiting]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `asked` reads the plan again after the person's word
  useEffect(() => {
    let current = true;
    let seen: string | undefined;
    const load = () =>
      api.plan(id).then(
        (p) => {
          if (!current) return;
          setPlan(p);
          setLoadFailure(null);
        },
        (f: Failure) => current && setLoadFailure(f),
      );
    load();
    const off = listen((e) => {
      if (e.type === 'lagged') load();
      if (e.type !== 'tasks' || e.chat !== chat) return;
      const now = JSON.stringify(e.plans?.find((p) => p.id === id));
      if (now !== seen) load();
      seen = now;
    });
    return () => {
      current = false;
      off();
    };
  }, [chat, id, asked]);

  const v = plan && (plan.versions.find((x) => x.n === version) ?? plan.versions.at(-1));
  if (!plan || !v) return loadFailure && <FailureNotice failure={loadFailure} onReference={onReference} />;
  const newest = v === plan.versions.at(-1);
  const open = ['checking', 'waiting', 'revising'].includes(plan.state);
  const waits = newest && plan.state === 'waiting';
  const decide = (call: () => Promise<unknown>) => {
    setBusy(true);
    setFailure(null);
    call()
      .then(() => setWords(null), setFailure)
      .finally(() => {
        setBusy(false);
        // Also after a refusal: the plan may have moved on in a live event this tab missed.
        setAsked((n) => n + 1);
      });
  };
  const meta = [name(plan.lead), began(Date.parse(v.created) * 1000), newest ? word[plan.state] : 'Replaced'];
  const check = checked(v, name);
  const before = plan.versions[plan.versions.indexOf(v) - 1];
  const marked = before && marks(v.parts, before.parts);
  return (
    <section className="tasks" aria-label="Plan">
      <div className="tasks-scroll">
        <div className="tasks-page plan-page">
          {(failure ?? loadFailure) && (
            <FailureNotice failure={(failure ?? loadFailure) as Failure} onReference={onReference} />
          )}
          <div className="plan-head">
            <p className="task-number">Plan · version {v.n}</p>
            {plan.versions.length > 1 && (
              <nav className="plan-versions" aria-label="Versions">
                {plan.versions.map((x, i) => (
                  <span key={x.n}>
                    {i > 0 && ' · '}
                    {x === v ? (
                      <span aria-current="page">v{x.n}</span>
                    ) : (
                      <button type="button" className="link" onClick={() => onVersion(x.n)}>
                        v{x.n}
                      </button>
                    )}
                  </span>
                ))}
              </nav>
            )}
          </div>
          <h2>{v.title}</h2>
          <p className="tasks-meta">
            {meta.filter(Boolean).map((m, i) => (
              <Fragment key={m}>
                {i > 0 && <span aria-hidden="true">·</span>}
                <span>{m}</span>
              </Fragment>
            ))}
          </p>
          {plan.asked && (
            <blockquote className="plan-asked">
              <small>You asked</small>
              {plan.asked}
            </blockquote>
          )}
          <p className="plan-text">{v.text}</p>
          <Checked plan={plan} version={v} before={before} check={check} name={name} />
          <h3>{counted(v.parts.length, 'part')}</h3>
          <ol className="plan-parts">
            {v.parts.map((p, i) => {
              const at = `${v.n}.${p.n}`;
              return (
                <Part
                  key={p.n}
                  part={p}
                  mark={marked?.parts[i]}
                  open={opened === at}
                  onToggle={() => setOpened(opened === at ? null : at)}
                  name={name}
                />
              );
            })}
            {marked?.removed.map((p) => (
              <li key={`removed ${p.n}`} className="plan-removed">
                <Mark word="Removed" /> <s>{p.what}</s>
              </li>
            ))}
          </ol>
          {waits && words !== null && (
            <ElicitationForm
              className="plan-back"
              presentation="inline"
              server=""
              message=""
              state="request"
              aria-label="Send back"
              fields={[
                {
                  name: 'words',
                  label: 'What to change',
                  kind: 'text',
                  value: words,
                  required: true,
                  multiline: true,
                },
              ]}
              onFieldChange={(_, value) => setWords(value)}
            />
          )}
        </div>
      </div>
      <footer className="tasks-footer">
        <div className="tasks-pager">
          {newest && open ? (
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => decide(() => api.dropPlan(id, v.n))}
            >
              Drop
            </Button>
          ) : (
            <span />
          )}
          <span role="status">{busy ? 'Saving' : ''}</span>
          <div>
            {waits &&
              (words === null ? (
                <>
                  <Button size="sm" variant="outline" disabled={busy} onClick={() => setWords('')}>
                    Send back
                  </Button>
                  <Button size="sm" disabled={busy} onClick={() => decide(() => api.approvePlan(id, v.n))}>
                    Approve
                  </Button>
                </>
              ) : (
                <>
                  <Button size="sm" variant="ghost" disabled={busy} onClick={() => setWords(null)}>
                    Cancel
                  </Button>
                  <Button
                    size="sm"
                    disabled={busy || !words.trim()}
                    onClick={() => decide(() => api.sendBackPlan(id, v.n, words))}
                  >
                    Send back
                  </Button>
                </>
              ))}
          </div>
        </div>
      </footer>
    </section>
  );
}

type Name = (agent: string | null) => string;

/** The check's outcome, this version's findings, its answers to the version before's, and the ask's questions. */
function Checked({
  plan,
  version: v,
  before,
  check,
  name,
}: {
  plan: Plan;
  version: PlanVersion;
  before?: PlanVersion;
  check?: string;
  name: Name;
}) {
  const findings = (v.check?.notes ?? []).filter((n) => n.kind !== 'suggestion');
  const answered = (note: number) => before?.check?.notes.find((n) => n.id === note);
  if (!check && !findings.length && !v.answers.length && !plan.questions.length && !plan.changed.length)
    return null;
  return (
    <dl className="task-fields">
      {check && (
        <>
          <dt>Check</dt>
          <dd>{check}</dd>
        </>
      )}
      {findings.length > 0 && (
        <>
          <dt>Findings</dt>
          {findings.map((n) => (
            <dd key={n.id}>{noteLine(n)}</dd>
          ))}
        </>
      )}
      {v.answers.length > 0 && (
        <>
          <dt>Answers</dt>
          {v.answers.map((a) => {
            const note = answered(a.note);
            return (
              <dd key={a.note}>
                <span>{note ? noteLine(note) : `Note ${a.note}`}</span>
                <span className="plan-said">
                  {a.changed ? 'Changed' : 'Kept'}: {a.why}
                </span>
              </dd>
            );
          })}
        </>
      )}
      {plan.questions.length > 0 && (
        <>
          <dt>Questions</dt>
          {plan.questions.map((q) => (
            <dd key={q.number}>
              <span>{q.what}</span>
              <span className="plan-said">{q.answer ?? 'Open'}</span>
            </dd>
          ))}
        </>
      )}
      {plan.changed.length > 0 && (
        <>
          <dt>Changed</dt>
          {plan.changed.map((c) => (
            <dd key={c.message}>
              {name(c.agent)}: {c.files.slice(0, NAMED).join(', ')}
              {c.files.length > NAMED && ` and ${c.files.length - NAMED} more`}
            </dd>
          ))}
        </>
      )}
    </dl>
  );
}

const Mark = ({ word }: { word: string }) => (
  <span className="plan-mark" data-mark={word}>
    {word}
  </span>
);

/** A part's brief, as its task will carry it word for word, each row with its field. */
const partRows = (p: PlanPart) => [
  ['Why', p.why, 'why'] as const,
  ...briefRows(p, [...p.after.map((n) => `part ${n}`), ...p.after_tasks.map((n) => `#${n}`)]),
];

interface PartProps {
  part: PlanPart;
  mark?: PartMark;
  open: boolean;
  onToggle: () => void;
  name: Name;
}

/** A part's line, which opens to its brief; a changed part's brief shows what each changed field was. */
function Part({ part: p, mark, open, onToggle, name }: PartProps) {
  const was = mark && 'was' in mark ? mark : undefined;
  const changed = (field: PartField | TaskField) => !!was?.changed.some((f) => f === field);
  const old = was && partRows(was.was);
  return (
    <li>
      <span className="plan-part-n">{p.n}</span>
      <div>
        <button type="button" className="plan-part-what" aria-expanded={open} onClick={onToggle}>
          {p.what}
          {mark && mark.mark !== 'Unchanged' && (
            <>
              {' '}
              <Mark word={mark.mark} />
            </>
          )}
        </button>
        <p className="plan-part-meta">{name(p.owner)}</p>
        {open && (
          <dl className="task-fields">
            {was && changed('owner') && (
              <>
                <dt>Was</dt>
                <dd className="plan-was">{name(was.was.owner)}</dd>
              </>
            )}
            {partRows(p).map(
              ([label, words, field]) =>
                (words || changed(field)) && (
                  <Fragment key={label}>
                    <dt>{label}</dt>
                    <dd>{words}</dd>
                    {changed(field) && (
                      <>
                        <dt>Was</dt>
                        <dd className="plan-was">{old?.find((r) => r[2] === field)?.[1]}</dd>
                      </>
                    )}
                  </Fragment>
                ),
            )}
          </dl>
        )}
      </div>
    </li>
  );
}

/** How the version's check stands, from the service's outcome. */
function checked(v: PlanVersion, name: Name): string | undefined {
  const c = v.check;
  if (!c) return undefined;
  const by = name(c.by);
  if (c.outcome === 'agrees') return `${by} agrees`;
  if (c.outcome === 'findings')
    return `${counted(c.notes.filter((n) => n.kind !== 'suggestion').length, 'finding')} from ${by}`;
  return c.outcome === null ? `${by} is checking` : `${by} didn't finish`;
}
