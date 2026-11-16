import { Fragment, useEffect, useState } from 'react';
import { ElicitationForm } from '@/components/assistant-ui/elements/elicitation-form';
import { Button } from '@/components/ui/button';
import { api, type Failure, type Plan, type PlanPart, type PlanVersion } from '../api';
import { began } from '../chat/team';
import { FailureNotice } from '../components/Notice';
import { listen } from '../live';
import { counted } from './plans';
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
}

/** The newest version's state word; waiting on the person has none. */
const word: Record<Plan['state'], string | undefined> = {
  checking: 'Checking',
  waiting: undefined,
  revising: 'Revising',
  approved: 'Approved',
  dropped: 'Dropped',
};

/**
 * The lead's plan in its own tab (0062 H5.4, the plan-card mock-up v2): a version whole, its check and every
 * part, with Approve, Send back and Drop at its foot. Read again whenever the chat's plans change.
 */
export function PlanTab({ chat, plan: id, version, name, onVersion, onReference }: Props) {
  const [plan, setPlan] = useState<Plan | null>(null);
  const [loadFailure, setLoadFailure] = useState<Failure | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [busy, setBusy] = useState(false);
  // Send back's words, while its field is open.
  const [words, setWords] = useState<string | null>(null);
  const [asked, setAsked] = useState(0);
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
          {check && (
            <dl className="task-fields">
              <dt>Check</dt>
              <dd>{check}</dd>
            </dl>
          )}
          <h3>{counted(v.parts.length, 'part')}</h3>
          <ol className="plan-parts">
            {v.parts.map((p) => (
              <Part key={p.n} part={p} name={name} />
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

/** A part shown whole: what its task will carry, word for word. */
function Part({ part: p, name }: { part: PlanPart; name: (agent: string | null) => string }) {
  const after = [...p.after.map((n) => `part ${n}`), ...p.after_tasks.map((n) => `#${n}`)].join(', ');
  const fields = [
    ['Why', p.why],
    ['Done when', p.done],
    ['Proof', p.proof],
    ['Where', p.where],
    ['Size', `${p.size}, about ${p.lines} lines`],
  ];
  return (
    <li>
      <span className="plan-part-n">{p.n}</span>
      <div>
        <p className="plan-part-what">{p.what}</p>
        <p className="plan-part-meta">
          {[name(p.owner), after && `after ${after}`].filter(Boolean).join(' · ')}
        </p>
        {fields.map(([label, value]) => (
          <p key={label}>
            <span>{label}:</span> {value}
          </p>
        ))}
      </div>
    </li>
  );
}

/** How the version's check stands, from the service's outcome. */
function checked(v: PlanVersion, name: (agent: string | null) => string): string | undefined {
  const c = v.check;
  if (!c) return undefined;
  const by = name(c.by);
  if (c.outcome === 'agrees') return `${by} agrees`;
  if (c.outcome === 'findings')
    return `${counted(c.notes.filter((n) => n.kind !== 'suggestion').length, 'finding')} from ${by}`;
  return c.outcome === null ? `${by} is checking` : `${by} didn't finish`;
}
