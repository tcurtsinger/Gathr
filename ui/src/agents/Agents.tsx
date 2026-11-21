import { Plus } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Account, type Agent, api, type Failure, type Model } from '../api';
import { Dropdown } from '../components/Dropdown';
import { Face } from '../components/Face';
import { FailureNotice } from '../components/Notice';
import { listen } from '../live';
import { useNewestWins } from '../newest';
import { Screen } from '../Screen';
import { AgentPage } from './AgentPage';
import { chosen } from './models';
import './agents.css';

const NEW = 'new';
const NONE = 'none';

interface Props {
  onReference: (ref: string) => void;
  /** The assistant was changed here: a new chat's greeting names it. */
  onAssistant: () => void;
}

/**
 * The Agents screen (agents/v1, 0026), in place of the chat list and the chat, as Plugins is: every agent in its
 * sidebar; the list with the assistant and Add; each agent's page with its account, model, effort and approvals.
 */
export function Agents({ onReference, onAssistant }: Props) {
  const [agents, setAgents] = useState<Agent[] | null>(null);
  const [accounts, setAccounts] = useState<Account[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  /** The agent whose page is open, NEW for Add, or null for the list. */
  const [open, setOpen] = useState<string | null>(null);
  const [assistant, setAssistant] = useState<string | null>(null);
  /** The agent last saved, so its page says so until it changes again. */
  const [saved, setSaved] = useState<string | null>(null);
  const newest = useNewestWins();

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
  const models = useListedModels(agents);

  const agent = agents?.find((a) => a.id === open);
  // An agent deleted elsewhere while its page is open: back to the list.
  useEffect(() => {
    if (open && open !== NEW && agents && !agents.some((a) => a.id === open)) setOpen(null);
  }, [open, agents]);
  const show = (id: string | null) => {
    setOpen(id);
    setSaved(null);
  };
  const pickAssistant = (value: string) => {
    const next = value === NONE ? null : value;
    setAssistant(next);
    setFailure(null);
    newest(api.saveSettings({ assistant: next })).then(
      (s) => {
        setAssistant(s.assistant ?? null);
        onAssistant();
      },
      (f: Failure) => {
        setFailure(f);
        newest(api.settings()).then((s) => setAssistant(s.assistant ?? null), setFailure);
      },
    );
  };
  const accountName = (id: string) => accounts?.find((a) => a.id === id)?.name ?? '';
  const ready = agents !== null && accounts !== null;
  const tag = <small className="agent-tag">Assistant</small>;

  return (
    <Screen
      name="Agents"
      nav={
        <div className="chatlist agent-nav">
          {agents?.length === 0 && <p className="sidebar-empty">Nothing yet</p>}
          {agents?.map((a) => (
            <div key={a.id} className="chatrow" data-current={a.id === open || undefined}>
              <button
                type="button"
                className="chat-link"
                aria-current={a.id === open ? 'page' : undefined}
                onClick={() => show(a.id)}
              >
                <Face agent={a} size={20} />
                <span>{a.name}</span>
                {a.id === assistant && tag}
              </button>
            </div>
          ))}
        </div>
      }
    >
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
      {ready && (agent || open === NEW) && accounts.length > 0 && (
        // A new page for each agent, so one agent's draft never carries to another.
        <AgentPage
          key={open}
          agent={agent}
          accounts={accounts}
          assistant={!!agent && agent.id === assistant}
          saved={!!agent && saved === agent.id}
          onBack={() => show(null)}
          onSaved={(done) => {
            setAgents((list) =>
              list?.some((a) => a.id === done.id)
                ? list.map((a) => (a.id === done.id ? done : a))
                : [...(list ?? []), done],
            );
            setOpen(done.id);
            setSaved(done.id);
          }}
          onDeleted={(left) => {
            setAgents((list) => list?.filter((a) => a.id !== open) ?? null);
            setFailure(left);
            show(null);
          }}
          onReference={onReference}
        />
      )}
      {ready && !agent && open !== NEW && (
        <>
          <h1>Agents</h1>
          <div className="screen-bar agent-bar">
            {agents.length > 0 && (
              <>
                <label htmlFor="agents-assistant" className="muted">
                  Assistant
                </label>
                <Dropdown
                  id="agents-assistant"
                  className="agent-assistant"
                  value={assistant ?? NONE}
                  onChange={pickAssistant}
                  choices={[
                    ...agents.map((a) => ({
                      value: a.id,
                      label: a.name,
                      icon: <Face agent={a} size={20} />,
                    })),
                    { value: NONE, label: 'None' },
                  ]}
                />
              </>
            )}
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="screen-add"
              disabled={accounts.length === 0}
              onClick={() => show(NEW)}
            >
              <Plus aria-hidden="true" />
              Add
            </Button>
          </div>
          {agents.length === 0 ? (
            <div className="screen-empty">
              <h2>No agents yet</h2>
              <p className="muted">
                Each agent runs on one of your accounts, with its own model and approvals.
              </p>
              {accounts.length === 0 && <p className="muted">No accounts</p>}
            </div>
          ) : (
            <ul className="agent-index">
              {agents.map((a) => {
                const listed = models[a.account];
                return (
                  <li key={a.id} className="screen-row">
                    <button type="button" className="screen-open" onClick={() => show(a.id)}>
                      <Face agent={a} size={28} />
                      <span className="screen-words">
                        <strong>
                          {a.name}
                          {a.id === assistant && tag}
                        </strong>
                        {a.description && <span>{a.description}</span>}
                      </span>
                      <span className="agent-meta">
                        {accountName(a.account)}
                        {listed && ` · ${chosen(listed, a.model)}`}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </>
      )}
    </Screen>
  );
}

/** The models of each account an agent runs on, as each engine lists them, for the list's rows; none where it fails. */
function useListedModels(agents: Agent[] | null) {
  const [models, setModels] = useState<Record<string, Model[]>>({});
  const used = [...new Set(agents?.map((a) => a.account))].join(' ');
  useEffect(() => {
    for (const account of used.split(' ').filter(Boolean))
      api.models(account).then(
        (list) => setModels((m) => ({ ...m, [account]: list })),
        () => {},
      );
  }, [used]);
  return models;
}
