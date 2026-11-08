import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { type Account, type Agent, api, type Failure, type Model } from '../api';
import { Notice } from '../components/Notice';
import { listen } from '../live';
import { report, timeToPaint } from '../report';
import { choices, chosen, efforts, firstLatest } from './models';
import './agents.css';

type Open = (ref: string) => void;
const NEW = 'new';

/** Settings > Agents (agents/v1, 0026): each agent's account, model and effort, edited in one place. */
export function Agents({ start, onReference }: { start?: string; onReference: Open }) {
  const [agents, setAgents] = useState<Agent[] | null>(null);
  const [accounts, setAccounts] = useState<Account[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [selected, setSelected] = useState<string | null>(start ?? null);
  /** The agent last saved, so its form says so until it changes again. */
  const [saved, setSaved] = useState<string | null>(null);
  const addButton = useRef<HTMLButtonElement>(null);

  const load = useCallback(() => {
    api.agents().then(setAgents, setFailure);
    api.accounts().then(setAccounts, setFailure);
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

  return (
    <section className="agents" aria-label="Agents">
      <div className="agent-list">
        <button
          type="button"
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
        </button>
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
            {a.name}
          </button>
        ))}
      </div>
      <div className="agent-form">
        {failure && (
          <Notice
            danger
            title={failure.error}
            reference={failure.ref ? { ref: failure.ref, open: onReference } : undefined}
            links={failure.why && <span className="muted">{failure.why}</span>}
          />
        )}
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
            onCancelNew={() => {
              setSelected(null);
              addButton.current?.focus();
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
  onReference: Open;
}

/** One Account picker (it implies the engine), then the account's models and their efforts. */
function AgentForm({ agent, accounts, saved, onSaved, onCancelNew, onReference }: FormProps) {
  const [name, setName] = useState(agent?.name ?? '');
  const [account, setAccount] = useState(agent?.account ?? accounts[0].id);
  /** Undefined until a new agent's models arrive and it takes the first Latest. */
  const [model, setModel] = useState<string | null | undefined>(agent ? agent.model : undefined);
  const [effort, setEffort] = useState(agent?.effort ?? null);
  const [find, setFind] = useState('');
  const [busy, setBusy] = useState(false);

  const [failure, setFailure] = useState<Failure | null>(null);
  const models = useModels(account);
  const title = useId();

  const list = models.list ?? [];
  useEffect(() => {
    if (model === undefined && models.list) setModel(firstLatest(models.list));
  }, [model, models.list]);

  const draft = { name: name.trim(), account, model: model ?? null, effort };
  const changed =
    !agent ||
    draft.name !== agent.name ||
    draft.account !== agent.account ||
    draft.model !== agent.model ||
    draft.effort !== agent.effort;
  const offered = efforts(list, draft.model);
  const found = choices(list).filter((c) => c.label.toLowerCase().includes(find.trim().toLowerCase()));

  const pickAccount = (id: string) => {
    setAccount(id);
    setModel(null);
    setEffort(null);
  };
  const pickModel = (id: string | null) => {
    setModel(id);
    if (effort && !efforts(list, id).includes(effort)) setEffort(null);
  };
  const reset = () => {
    if (!agent) return onCancelNew();
    setAccount(agent.account);
    setModel(agent.model);
    setEffort(agent.effort);
    setFind('');
  };
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setFailure(null);
    const call = agent ? api.saveAgent({ id: agent.id, ...draft }) : api.addAgent(draft);
    call.then(
      (done) => {
        setBusy(false);
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
      {!agent && (
        <label className="agent-field">
          Name
          <input
            value={name}
            maxLength={80}
            autoComplete="off"
            spellCheck={false}
            // biome-ignore lint/a11y/noAutofocus: the form opens from Add, to type the name
            autoFocus
            onChange={(e) => setName(e.target.value)}
          />
        </label>
      )}
      <label className="agent-field">
        Account
        <select value={account} onChange={(e) => pickAccount(e.target.value)}>
          {accounts.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name} / Claude Code{a.login === 'signed_out' ? ' / Signed out' : ''}
            </option>
          ))}
        </select>
      </label>
      <fieldset className="agent-field">
        <legend>Model</legend>
        <p className="agent-chosen" aria-live="polite">
          {model === undefined ? '' : chosen(list, model)}
        </p>
        {models.failure ? (
          <Notice
            danger
            title={models.failure.error}
            reference={models.failure.ref ? { ref: models.failure.ref, open: onReference } : undefined}
            links={models.failure.why && <span className="muted">{models.failure.why}</span>}
          />
        ) : models.list === null ? (
          <p className="muted" role="status">
            Loading models
          </p>
        ) : (
          <>
            <input
              type="search"
              aria-label="Find models"
              placeholder="Find models"
              value={find}
              onChange={(e) => setFind(e.target.value)}
            />
            <fieldset className="model-results" aria-label="Models">
              {found.map((c) => (
                <button
                  type="button"
                  key={c.id ?? ''}
                  aria-pressed={c.id === (model ?? null)}
                  onClick={() => pickModel(c.id)}
                >
                  {c.label}
                </button>
              ))}
              {found.length === 0 && <p className="muted">No models</p>}
            </fieldset>
          </>
        )}
      </fieldset>
      <label className="agent-field">
        Effort
        <select value={effort ?? ''} onChange={(e) => setEffort(e.target.value || null)}>
          <option value="">Engine default</option>
          {offered.map((level) => (
            <option key={level} value={level}>
              {level}
            </option>
          ))}
        </select>
      </label>
      {failure && (
        <Notice
          danger
          title={failure.error}
          reference={failure.ref ? { ref: failure.ref, open: onReference } : undefined}
          links={failure.why && <span className="muted">{failure.why}</span>}
        />
      )}
      <div className="agent-actions">
        <span className="muted" role="status">
          {saved && !changed ? 'Saved' : ''}
        </span>
        <button type="button" onClick={reset} disabled={busy || (!!agent && !changed)}>
          Cancel
        </button>
        <button
          type="submit"
          className="primary"
          disabled={busy || !changed || !draft.name || model === undefined}
        >
          Save
        </button>
      </div>
    </form>
  );
}

/** The account's models as its engine lists them, asked again when the account changes. */
function useModels(account: string) {
  const [state, setState] = useState<{ list: Model[] | null; failure: Failure | null }>({
    list: null,
    failure: null,
  });
  useEffect(() => {
    let current = true;
    const started = performance.now();
    setState({ list: null, failure: null });
    api.models(account).then(
      (list) => {
        if (!current) return;
        setState({ list, failure: null });
        timeToPaint('gathr.ui.models', started, { account, models: list.length });
      },
      (failure: Failure) => {
        if (!current) return;
        setState({ list: null, failure });
        report({ name: 'gathr.ui.models', error: failure.error, attrs: { account, ref: failure.ref } });
      },
    );
    return () => {
      current = false;
    };
  }, [account]);
  return state;
}
