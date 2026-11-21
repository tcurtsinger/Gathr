import { Fragment, useCallback, useEffect, useId, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Account, type Agent, type Approvals, api, type Failure, type Model } from '../api';
import { danger } from '../components/Confirm';
import { Dropdown } from '../components/Dropdown';
import { Help } from '../components/Help';
import { FailureNotice } from '../components/Notice';
import { DeleteConfirm } from '../DeleteConfirm';
import { listen } from '../live';
import { report, timeToPaint } from '../report';
import { engineName } from './Accounts';
import { isCustom } from './BackgroundField';
import { FaceFields } from './FaceFields';
import { choices, chosen, efforts, firstChoice } from './models';
import './agents.css';

type Open = (ref: string) => void;
const NEW = 'new';

/** Settings > Agents (agents/v1, 0026): each agent's account, model and effort, edited in one place. */
export function Agents({ onReference }: { onReference: Open }) {
  const [agents, setAgents] = useState<Agent[] | null>(null);
  const [accounts, setAccounts] = useState<Account[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  /** The assistant's id, marked in the list. */
  const [assistant, setAssistant] = useState<string | null>(null);
  /** The agent last saved, so its form says so until it changes again. */
  const [saved, setSaved] = useState<string | null>(null);
  const addButton = useRef<HTMLButtonElement>(null);

  const load = useCallback(() => {
    api.agents().then((a) => setAgents(a.agents), setFailure);
    api.accounts().then(setAccounts, setFailure);
    api.settings().then((s) => setAssistant(s.assistant ?? null), setFailure);
  }, []);
  useEffect(() => {
    load();
    return listen((event) => {
      if (event.type === 'agents' || event.type === 'accounts' || event.type === 'lagged') load();
    });
  }, [load]);

  const shown = selected ?? agents?.[0]?.id ?? null;
  const agent = agents?.find((a) => a.id === shown);
  const ready = agents !== null && accounts !== null;
  const toAdd = () => {
    setSelected(null);
    addButton.current?.focus();
  };

  return (
    <section className="agents" aria-label="Agents">
      <div className="agent-list">
        <Button
          type="button"
          variant="outline"
          ref={addButton}
          className="agent-add"
          disabled={accounts?.length === 0}
          aria-current={shown === NEW ? 'true' : undefined}
          onClick={() => {
            setSelected(NEW);
            setSaved(null);
          }}
        >
          Add
        </Button>
        {agents?.map((a) => (
          <button
            type="button"
            key={a.id}
            aria-current={a.id === shown ? 'true' : undefined}
            onClick={() => {
              setSelected(a.id);
              setSaved(null);
            }}
          >
            <span>{a.name}</span>
            {a.id === assistant && <small>Assistant</small>}
          </button>
        ))}
      </div>
      <div className="agent-form">
        {failure && <FailureNotice failure={failure} onReference={onReference} />}
        {ready && accounts.length === 0 && <p className="muted">No accounts</p>}
        {ready && accounts.length > 0 && !agent && shown !== NEW && <p className="muted">No agents</p>}
        {ready && accounts.length > 0 && (agent || shown === NEW) && (
          // A new form for each agent, so one agent's draft never carries to another.
          <AgentForm
            key={shown}
            agent={agent}
            accounts={accounts}
            saved={!!agent && saved === agent.id}
            onSaved={(done) => {
              setAgents((list) =>
                list?.some((a) => a.id === done.id)
                  ? list.map((a) => (a.id === done.id ? done : a))
                  : [...(list ?? []), done],
              );
              setSelected(done.id);
              setSaved(done.id);
            }}
            onCancelNew={toAdd}
            onDeleted={(left) => {
              setAgents((list) => list?.filter((a) => a.id !== shown) ?? null);
              setFailure(left);
              toAdd();
            }}
            onReference={onReference}
          />
        )}
      </div>
    </section>
  );
}

interface FormProps {
  agent?: Agent;
  accounts: Account[];
  saved: boolean;
  onSaved: (agent: Agent) => void;
  onCancelNew: () => void;
  /** What the delete couldn't remove yet, if anything. */
  onDeleted: (left: Failure | null) => void;
  onReference: Open;
}

/** Model is undefined until its account's models arrive: a new agent, or one moved to another account, takes the first. */
type Fields = {
  name: string;
  description?: string | null;
  account: string;
  model: string | null | undefined;
  effort: string | null;
  approvals: Approvals;
  /** Undefined for a new agent until Shuffle: its face comes from its name. */
  face_seed?: string;
  skin: number | null;
  background: string | null;
  picture?: string | null;
  /** Background's last colour not among the twelve, while the form is open: never sent. */
  custom?: string;
};

const modes: [Approvals, string, string][] = [
  ['ask', 'Ask', 'Ask before actions that need permission'],
  ['edits', 'Edits', 'Allow file edits in the worktree. Ask before other actions that need permission.'],
  [
    'auto',
    'Auto',
    'The engine checks actions. Pushes and file tools writing outside the worktree need approval.',
  ],
  ['full', 'Full', 'Allow actions without asking'],
];

/** One Account picker (it implies the engine), then the account's models and their efforts. */
function AgentForm(props: FormProps) {
  const { agent, accounts, saved, onSaved, onCancelNew, onDeleted, onReference } = props;
  // Only the fields edited here, over the agent as saved, so a save from elsewhere shows in
  // every field not edited here and Save never writes an untouched field back.
  const [edits, setEdits] = useState<Partial<Fields>>({});
  const edit = (fields: Partial<Fields>) => setEdits((e) => ({ ...e, ...fields }));
  const blank: Fields = {
    name: '',
    account: accounts[0].id,
    model: undefined,
    effort: null,
    approvals: 'auto',
    skin: null,
    background: null,
  };
  const {
    name,
    description,
    account,
    model,
    effort,
    approvals,
    face_seed,
    skin,
    background,
    picture = null,
  }: Fields = {
    ...(agent ?? blank),
    ...edits,
  };
  const seed = face_seed ?? name.trim();
  const [busy, setBusy] = useState(false);
  /** Delete's button while its confirm is open: focus goes back to it on Cancel. */
  const [deleting, setDeleting] = useState<HTMLElement | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const models = useModels(account);
  const title = useId();
  const approvalsId = useId();

  const list = models.list ?? [];
  useEffect(() => {
    const loaded = models.list;
    // With no exact model listed, none is chosen and Save stays off: never the engine's default.
    const first = loaded && firstChoice(loaded);
    if (model === undefined && first) setEdits((e) => ({ ...e, model: first }));
  }, [model, models.list]);

  // A face field goes with the save only when it changed, so Save never writes an untouched one back.
  const face = {
    ...(seed !== (agent?.face_seed ?? name.trim()) && { face_seed: seed }),
    ...(skin !== (agent?.skin ?? null) && { skin }),
    ...(background !== (agent?.background ?? null) && { background }),
    ...(picture !== (agent?.picture ?? null) && { picture }),
  };
  // The description too: sent only when it changed.
  const about = description?.trim() || null;
  const described = about !== (agent?.description ?? null) && { description: about };
  const draft = {
    name: name.trim(),
    account,
    model: model ?? null,
    effort,
    approvals,
    ...face,
    ...described,
  };
  const changed =
    !agent ||
    Object.keys(face).length > 0 ||
    !!described ||
    draft.name !== agent.name ||
    draft.account !== agent.account ||
    draft.model !== agent.model ||
    draft.effort !== agent.effort ||
    draft.approvals !== agent.approvals;
  const offered = efforts(list, draft.model);
  const exact = choices(list).map((c) => ({ value: c.id, label: c.label }));

  const pickAccount = (id: string) => edit({ account: id, model: undefined, effort: null });
  const pickModel = (id: string) =>
    edit(effort && !efforts(list, id).includes(effort) ? { model: id, effort: null } : { model: id });
  const reset = () => {
    if (!agent) return onCancelNew();
    setEdits({});
  };
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setFailure(null);
    const call = agent ? api.saveAgent({ id: agent.id, ...draft }) : api.addAgent(draft);
    call.then(
      (done) => {
        setBusy(false);
        setEdits({});
        onSaved(done);
      },
      (f: Failure) => {
        setBusy(false);
        setFailure(f);
      },
    );
  };

  return (
    <form aria-labelledby={title} onSubmit={submit}>
      <h3 id={title}>{agent ? agent.name : 'Add agent'}</h3>
      <label className="agent-field">
        Name
        <input
          value={name}
          maxLength={80}
          autoComplete="off"
          spellCheck={false}
          // biome-ignore lint/a11y/noAutofocus: the form opens from Add, to type the name
          autoFocus={!agent}
          onChange={(e) => edit({ name: e.target.value })}
        />
      </label>
      <label className="agent-field">
        Description
        <input
          value={description ?? ''}
          maxLength={80}
          autoComplete="off"
          onChange={(e) => edit({ description: e.target.value })}
        />
      </label>
      <FaceFields
        id={agent?.id ?? ''}
        name={name.trim()}
        seed={seed}
        skin={skin}
        background={background}
        picture={picture}
        remembered={edits.custom ?? (isCustom(agent?.background) ? agent.background : null)}
        onChange={edit}
      />
      <div className="agent-field">
        <label htmlFor={`${title}account`}>Account</label>
        <Dropdown
          id={`${title}account`}
          value={account}
          onChange={pickAccount}
          choices={accounts.map((a) => ({
            value: a.id,
            label: `${a.name} / ${engineName(a.engine)}${a.login === 'signed_out' ? ' / Signed out' : ''}`,
          }))}
        />
      </div>
      <div className="agent-field">
        <label htmlFor={`${title}model`}>Model</label>
        {models.failure ? (
          <FailureNotice failure={models.failure} onReference={onReference} />
        ) : models.list === null ? (
          <p className="muted" role="status">
            Loading models
          </p>
        ) : (
          <Dropdown
            id={`${title}model`}
            // Only an exact model is chosen in the list: an agent on a Latest or the engine default reads as the
            // model that is today, and picking that model saves it exactly.
            value={model ?? ''}
            shown={model === undefined ? undefined : chosen(list, model)}
            placeholder={exact.length > 0 ? undefined : 'No models'}
            onChange={pickModel}
            choices={exact}
          />
        )}
      </div>
      <div className="agent-field">
        <label htmlFor={`${title}effort`}>Effort</label>
        <Dropdown
          id={`${title}effort`}
          value={effort ?? ''}
          onChange={(v) => edit({ effort: v || null })}
          choices={[
            { value: '', label: 'Engine default' },
            ...offered.map((level) => ({ value: level, label: level })),
            ...(effort && !offered.includes(effort)
              ? [{ value: effort, label: `${effort} / Unavailable` }]
              : []),
          ]}
        />
      </div>
      <div className="agent-field">
        <span className="agent-label">
          <label htmlFor={approvalsId}>Approvals</label>
          <Help label="About approvals">
            <h4>Approvals</h4>
            <dl>
              {modes.map(([mode, label, detail]) => (
                <Fragment key={mode}>
                  <dt>{label}</dt>
                  <dd>{detail}</dd>
                </Fragment>
              ))}
            </dl>
            {/* Codex's sandbox keeps commands' writes in the worktree (0033). */}
            {accounts.find((a) => a.id === account)?.engine === 'claude' && (
              <p>Shell commands can run outside the worktree in Auto mode</p>
            )}
          </Help>
        </span>
        <Dropdown
          id={approvalsId}
          value={approvals}
          onChange={(v) => edit({ approvals: v as Approvals })}
          choices={modes.map(([mode, label]) => ({ value: mode, label }))}
        />
      </div>
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
      <div className="agent-actions">
        {agent && (
          <Button
            type="button"
            variant="outline"
            className={danger}
            disabled={busy}
            onClick={(e) => setDeleting(e.currentTarget)}
          >
            Delete
          </Button>
        )}
        {agent && deleting && (
          <DeleteConfirm
            kind="agent"
            id={agent.id}
            name={agent.name}
            opener={deleting}
            onCancel={() => setDeleting(null)}
            onDeleted={onDeleted}
            onReference={onReference}
          />
        )}
        <span className="muted" role="status">
          {saved && !changed ? 'Saved' : ''}
        </span>
        <Button type="button" variant="ghost" onClick={reset} disabled={busy || (!!agent && !changed)}>
          Cancel
        </Button>
        <Button type="submit" disabled={busy || !changed || !draft.name || model === undefined}>
          Save
        </Button>
      </div>
    </form>
  );
}

/** The account's models as its engine lists them, asked again when the account changes. */
const NO_MODELS = { list: null as Model[] | null, failure: null as Failure | null };

function useModels(account: string) {
  const [state, setState] = useState({ of: account, ...NO_MODELS });
  useEffect(() => {
    let current = true;
    const started = performance.now();
    setState({ of: account, ...NO_MODELS });
    api.models(account).then(
      (list) => {
        if (!current) return;
        setState({ of: account, list, failure: null });
        timeToPaint('gathr.ui.models', started, { account, models: list.length });
      },
      (failure: Failure) => {
        if (!current) return;
        setState({ of: account, list: null, failure });
        report({ name: 'gathr.ui.models', error: failure.error, attrs: { account, ref: failure.ref } });
      },
    );
    return () => {
      current = false;
    };
  }, [account]);
  // Never the account before's list: a model is picked from it as soon as one is there.
  return state.of === account ? state : NO_MODELS;
}
