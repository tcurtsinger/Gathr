import { Fragment, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  api,
  type Failure,
  type Plan,
  type PlanPart,
  type PlanVersion,
  type Task,
  type TaskField,
} from '../api';
import { began } from '../chat/team';
import { Face } from '../components/Face';
import { Markdown } from '../components/Markdown';
import { FailureNotice } from '../components/Notice';
import { listen } from '../live';
import { briefRows } from '../tasks/model';
import { Note } from '../tasks/Note';
import { Field, Refused } from '../tasks/Question';
import { Line, Row, TaskRow } from '../tasks/TaskList';
import { counted, marks, type PartField, type PartMark } from './plans';
import '../tasks/tasks.css';
import './plans.css';

interface Props {
  chat: string;
  plan: number;
  /** The version shown; the newest when the plan has no such version yet. */
  version: number;
  name: (agent: string | null) => string;
  /** The chat's tasks: once approved, a part is the task it became. */
  tasks: Task[];
  onOpenTask: (task: number) => void;
  onVersion: (version: number) => void;
  onReference: (ref: string) => void;
  /** Whether the plan waits on the person: the tab's dot. */
  onWaits?: (waits: boolean) => void;
}

/** The newest version's state, in a word under the title. */
const WORD: Record<Plan['state'], string> = {
  checking: 'Checking',
  waiting: 'Waiting',
  revising: 'Revising',
  approved: 'Approved',
  dropped: 'Dropped',
};

/** The files a catch names, as Gathr's line in the chat does; the rest are counted. */
const NAMED = 20;

/**
 * The lead's plan in its own tab (0062 H5.4, the plan-tab mock-up): a version as a task's page, its check with
 * the findings and answers, the ask's questions, and its parts as Tasks rows, one open at a time on a tint and
 * marked against the version before; once approved, the tasks they became. Its foot holds only what acts:
 * Drop, Send back and Approve, and a refusal over them. Read again whenever the chat's plans change.
 */
export function PlanTab(props: Props) {
  const { chat, plan: id, version, name, tasks, onOpenTask, onVersion, onReference, onWaits } = props;
  const [plan, setPlan] = useState<Plan | null>(null);
  const [loadFailure, setLoadFailure] = useState<Failure | null>(null);
  const [refused, setRefused] = useState<{ word: string; failure: Failure } | null>(null);
  const [busy, setBusy] = useState(false);
  // Send back's words, while its field is open.
  const [words, setWords] = useState<string | null>(null);
  const [asked, setAsked] = useState(0);
  // The one part open to its brief, by its version and number: another version opens with none open.
  const [opened, setOpened] = useState<string | null>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const foot = useRef<HTMLElement>(null);
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
  const newest = !!plan && v === plan.versions.at(-1);
  const toTop = () => {
    if (scroller.current) scroller.current.scrollTop = 0;
  };
  // Another version opens at its top. A state changed from elsewhere (a check that ends) keeps the place.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the version shown is the reason, not a value read
  useLayoutEffect(toTop, [v?.n]);
  const asking = words !== null;
  useEffect(() => {
    if (asking) foot.current?.querySelector('textarea')?.focus();
  }, [asking]);

  if (!plan || !v) return loadFailure && <FailureNotice failure={loadFailure} onReference={onReference} />;
  const word = newest ? WORD[plan.state] : 'Replaced';
  const open = ['checking', 'waiting', 'revising'].includes(plan.state);
  const waits = newest && plan.state === 'waiting';
  const drop = newest && open;
  const decide = (call: () => Promise<unknown>, refusedAs: string) => {
    setBusy(true);
    setRefused(null);
    call()
      .then(
        () => {
          // The person's own word went through: what the tab holds changes with it.
          setWords(null);
          toTop();
        },
        (failure: Failure) => setRefused({ word: refusedAs, failure }),
      )
      .finally(() => {
        setBusy(false);
        // Also after a refusal: the plan may have moved on in a live event this tab missed.
        setAsked((n) => n + 1);
      });
  };
  const sendBack = () => {
    if (!busy && words?.trim()) decide(() => api.sendBackPlan(id, v.n, words), 'Not sent back');
  };
  const before = plan.versions[plan.versions.indexOf(v) - 1];
  const marked = before && marks(v.parts, before.parts);
  const lead = (
    <span key="lead" className="task-owner">
      <Face agent={{ id: plan.lead, name: name(plan.lead) }} size={16} />
      {name(plan.lead)}
    </span>
  );
  return (
    <section className="tasks" aria-label="Plan">
      <div className="tasks-scroll" ref={scroller}>
        <div className="tasks-page plan-page">
          {loadFailure && <FailureNotice failure={loadFailure} onReference={onReference} />}
          <div className="tasks-top">
            <span>Version {v.n}</span>
            {plan.versions.length > 1 && (
              <nav className="plan-versions" aria-label="Versions">
                {plan.versions.map((x, i) => (
                  <span key={x.n}>
                    {i > 0 && <span aria-hidden="true">·</span>}
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
          <Line
            className="task-view-line"
            pieces={[
              lead,
              began(Date.parse(v.created) * 1000),
              word === 'Waiting' ? (
                <span key="word" className="task-state you">
                  Waiting
                </span>
              ) : (
                word
              ),
            ]}
          />
          <Markdown text={v.text} className="plan-text" />
          <Fields plan={plan} version={v} before={before} name={name} />
          <div className="task-list plan-parts">
            <section aria-label="Parts">
              <h3>
                Parts<span>{v.parts.length}</span>
              </h3>
              <ul className="task-rows">
                {v.parts.map((p, i) => {
                  const made = p.task === null ? undefined : tasks.find((t) => t.number === p.task);
                  const at = `${v.n}.${p.n}`;
                  return (
                    <li key={p.n} className={opened === at && !made ? 'plan-open' : undefined}>
                      {made ? (
                        <TaskRow task={made} tasks={tasks} name={name} onOpen={onOpenTask} />
                      ) : (
                        <Part
                          part={p}
                          mark={marked?.parts[i]}
                          before={before?.parts ?? []}
                          parts={v.parts}
                          open={opened === at}
                          onToggle={() => setOpened(opened === at ? null : at)}
                          name={name}
                        />
                      )}
                    </li>
                  );
                })}
                {marked?.removed.map((p) => (
                  <li key={`removed ${p.n}`}>
                    <Row
                      face={<Face agent={{ id: p.owner, name: name(p.owner) }} size={24} />}
                      what={<s>{p.what}</s>}
                      line={[name(p.owner), 'Removed']}
                      ended
                    />
                  </li>
                ))}
              </ul>
            </section>
          </div>
        </div>
      </div>
      {(drop || waits || refused) && (
        <footer className="tasks-footer plan-foot" ref={foot}>
          <div className="plan-foot-in">
            {refused && (
              <div className="plan-refused">
                <Refused
                  word={refused.word}
                  refusal={{ why: refused.failure.why ?? refused.failure.error, ref: refused.failure.ref }}
                  onReference={onReference}
                />
              </div>
            )}
            {waits && asking && (
              <Field
                place="What to change"
                value={words}
                onType={setWords}
                ready={!busy && !!words.trim()}
                onSend={sendBack}
                arrow={false}
              />
            )}
            {drop && (
              <Button
                size="sm"
                variant="outline"
                className="plan-drop"
                disabled={busy}
                onClick={() => decide(() => api.dropPlan(id, v.n), 'Not dropped')}
              >
                Drop
              </Button>
            )}
            {waits && (
              <div className="plan-acts">
                {asking ? (
                  <>
                    <Button size="sm" variant="outline" disabled={busy} onClick={() => setWords(null)}>
                      Cancel
                    </Button>
                    <Button size="sm" disabled={busy || !words.trim()} onClick={sendBack}>
                      Send back
                    </Button>
                  </>
                ) : (
                  <>
                    <Button size="sm" variant="outline" disabled={busy} onClick={() => setWords('')}>
                      Send back
                    </Button>
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() => decide(() => api.approvePlan(id, v.n), 'Not approved')}
                    >
                      Approve
                    </Button>
                  </>
                )}
              </div>
            )}
          </div>
        </footer>
      )}
    </section>
  );
}

type Name = (agent: string | null) => string;

/**
 * The plan's fields: what was asked, how the check stands, this version's findings, its answers to the version
 * before's, the ask's questions, the person's words if it was sent back, and what held turns changed.
 */
function Fields({
  plan,
  version: v,
  before,
  name,
}: {
  plan: Plan;
  version: PlanVersion;
  before?: PlanVersion;
  name: Name;
}) {
  const check = checked(v, name);
  const findings = (v.check?.notes ?? []).filter((n) => n.kind !== 'suggestion');
  const answered = (note: number) => before?.check?.notes.find((n) => n.id === note);
  return (
    <dl className="task-fields plan-fields">
      {plan.asked && (
        <>
          <dt>Asked</dt>
          <dd className="task-why">{plan.asked}</dd>
        </>
      )}
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
            <dd key={n.id}>
              <Note note={n} />
            </dd>
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
                {note ? <Note note={note} /> : <span>Note {a.note}</span>}
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
      {v.words && (
        <>
          <dt>Sent back</dt>
          <dd className="task-why">{v.words}</dd>
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

/** "Part 1", "Parts 3, 4". */
const partsNamed = (ns: number[]) => (ns.length ? `Part${ns.length > 1 ? 's' : ''} ${ns.join(', ')}` : '');
/** What a part comes after, in its brief: "Parts 3, 4, #41". */
const afterWords = (p: PlanPart) =>
  [partsNamed(p.after), ...p.after_tasks.map((n) => `#${n}`)].filter(Boolean).join(', ');

/**
 * What a changed part came after, named in this version: a part still here by its number now, a part that went
 * by its what, so an old number never reads as this version's part of that number.
 */
function afterWas(was: PlanPart, before: PlanPart[], parts: PlanPart[]) {
  const here: number[] = [];
  const gone: string[] = [];
  for (const n of was.after) {
    const what = before.find((b) => b.n === n)?.what.trim();
    const now = parts.find((p) => p.what.trim() === what);
    if (now) here.push(now.n);
    else if (what) gone.push(`${what}, removed`);
  }
  here.sort((a, b) => a - b);
  return [partsNamed(here), ...gone, ...was.after_tasks.map((n) => `#${n}`)].filter(Boolean).join(', ');
}

/** A part's brief, as its task will carry it word for word, each row with its field. */
const partRows = (p: PlanPart, after: string) => [
  ['Why', p.why, 'why'] as const,
  ...briefRows(p, after ? [after] : []),
];

interface PartProps {
  part: PlanPart;
  mark?: PartMark;
  /** The version before's parts, and this version's: a changed After names its parts by them. */
  before: PlanPart[];
  parts: PlanPart[];
  open: boolean;
  onToggle: () => void;
  name: Name;
}

/**
 * A part as a Tasks row, "Part 2 · Ann · After 1 · Changed", which opens to its brief; a changed field's old
 * words sit under its new ones, and a changed owner gets an Owner row.
 */
function Part({ part: p, mark, before, parts, open, onToggle, name }: PartProps) {
  const was = mark && 'was' in mark ? mark : undefined;
  const changed = (field: PartField | TaskField) => !!was?.changed.some((f) => f === field);
  const old = (field: PartField | TaskField) => {
    if (!was) return undefined;
    if (field === 'owner') return name(was.was.owner);
    if (field === 'after') return afterWas(was.was, before, parts);
    return partRows(was.was, '').find((r) => r[2] === field)?.[1];
  };
  const after = afterWords(p);
  const row = (label: string, words: string | null | undefined, field: PartField | TaskField) => (
    <Fragment key={label}>
      <dt>{label}</dt>
      <dd>
        {words && <Markdown text={words} />}
        {changed(field) && <span className="plan-was">Was: {old(field) || 'none'}</span>}
      </dd>
    </Fragment>
  );
  return (
    <>
      <Row
        face={<Face agent={{ id: p.owner, name: name(p.owner) }} size={24} />}
        what={p.what}
        expanded={open}
        onClick={onToggle}
        line={[
          `Part ${p.n}`,
          name(p.owner),
          after && `After ${[...p.after, ...p.after_tasks.map((n) => `#${n}`)].join(', ')}`,
          mark && mark.mark !== 'Unchanged' && (
            <span key="mark" className="plan-mark">
              {mark.mark}
            </span>
          ),
        ]}
      />
      {open && (
        <dl className="task-fields plan-brief">
          {changed('owner') && row('Owner', name(p.owner), 'owner')}
          {partRows(p, after).map(
            ([label, words, field]) => (words || changed(field)) && row(label, words, field),
          )}
        </dl>
      )}
    </>
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
