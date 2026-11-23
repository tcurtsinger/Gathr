import { Plus } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Account, type Agent, api, type Failure } from '../api';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { WORD } from '../components/marks';
import { StateMark } from '../components/StateMark';
import { listen } from '../live';
import { useNewestWins } from '../newest';
import type { AgentsAt, ProposingAgent } from '../room/Proposal';
import { Screen } from '../Screen';
import { AgentPage } from './AgentPage';
import './agents.css';

const NEW = 'new';

interface Props {
  /** Where it opens from an agent the assistant proposed: that agent's page, or Add's form filled in with it. */
  at?: AgentsAt;
  /** The assistant was changed here: a new chat's greeting names it. */
  onAssistant: () => void;
}

/**
 * The Agents screen (agents/v1, 0026), in place of the chat list and the chat, as Plugins is: every agent in its
 * sidebar; the list with the assistant and Add; each agent's page with its account, model, effort and approvals.
 */
export function Agents({ at, onAssistant }: Props) {
  const [agents, setAgents] = useState<Agent[] | null>(null);
  const [accounts, setAccounts] = useState<Account[] | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  /** The agent whose page is open, NEW for Add, or null for the list. */
  const [open, setOpen] = useState<string | null>(() => (at ? ('agent' in at ? at.agent : NEW) : null));
  const [assistant, setAssistant] = useState<string | null>(null);
  /** The agent last saved, so its page says so until it changes again. */
  const [saved, setSaved] = useState<string | null>(null);
  /** The proposal Add's form is filled in with, while it is open. */
  const [proposal, setProposal] = useState<ProposingAgent | null>(() =>
    at && 'proposal' in at ? at.proposal : null,
  );
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

  // Nothing picked: the assistant's page, else the first agent's. The list is the sidebar, never a page of its own.
  const current = open ?? assistant ?? agents?.[0]?.id ?? null;
  const agent = agents?.find((a) => a.id === current);
  // An agent deleted elsewhere while its page is open: back to the first.
  useEffect(() => {
    if (open && open !== NEW && agents && !agents.some((a) => a.id === open)) setOpen(null);
  }, [open, agents]);
  const show = (id: string | null) => {
    setOpen(id);
    setSaved(null);
    setProposal(null);
  };
  const pickAssistant = (next: string) => {
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
  const ready = agents !== null && accounts !== null;
  const add = (
    <Button
      type="button"
      variant="outline"
      size="sm"
      className="screen-add"
      disabled={!accounts?.length}
      onClick={() => show(NEW)}
    >
      <Plus aria-hidden="true" />
      Add
    </Button>
  );

  return (
    <Screen
      name="Agents"
      action={agents?.length ? add : undefined}
      nav={
        <div className="chatlist agent-nav">
          {agents?.map((a) => (
            <div key={a.id} className="chatrow" data-current={a.id === current || undefined}>
              <button
                type="button"
                className="chat-link"
                aria-current={a.id === current ? 'page' : undefined}
                onClick={() => show(a.id)}
              >
                <Face agent={a} size={20} />
                <span>{a.name}</span>
                {a.id === assistant && <small className="agent-tag">Assistant</small>}
                {accounts?.find((x) => x.id === a.account)?.login === 'signed_out' && (
                  // Its account is signed out: the mark the sidebar's assistant row shows.
                  <i className="chat-mark" role="img" aria-label={WORD.cant}>
                    <StateMark state="cant" size={9} />
                  </i>
                )}
              </button>
            </div>
          ))}
        </div>
      }
    >
      {ready && (agent || current === NEW) && accounts.length > 0 && (
        // A new page for each agent, so one agent's draft never carries to another.
        <AgentPage
          key={agent ? current : (proposal?.message ?? current)}
          agent={agent}
          accounts={accounts}
          assistant={!!agent && agent.id === assistant}
          saved={!!agent && saved === agent.id}
          proposal={agent ? null : proposal}
          onBack={() => show(null)}
          onAssistant={() => agent && pickAssistant(agent.id)}
          onSaved={(done) => {
            setAgents((list) =>
              list?.some((a) => a.id === done.id)
                ? list.map((a) => (a.id === done.id ? done : a))
                : [...(list ?? []), done],
            );
            setOpen(done.id);
            setSaved(done.id);
            setProposal(null);
          }}
          onDeleted={(left) => {
            setAgents((list) => list?.filter((a) => a.id !== current) ?? null);
            setFailure(left);
            show(null);
          }}
        />
      )}
      {ready && agents.length === 0 && current !== NEW && (
        <div className="screen-empty">
          <h2>No agents yet</h2>
          <p className="muted">
            {accounts.length
              ? 'Each has an account, a model and approvals.'
              : 'Add an account in Settings first.'}
          </p>
          {add}
        </div>
      )}
    </Screen>
  );
}
