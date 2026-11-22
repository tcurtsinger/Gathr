import { Folder } from 'lucide-react';
import { createContext, useContext, useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { chosen } from '../agents/models';
import { type Account, type Agent, api, type Failure, type Model } from '../api';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { AgentsMark } from '../Rail';
import { engineName } from '../settings/Accounts';
import './proposal.css';

/** A project the assistant proposed with `propose_project`, as its block keeps it; once made, what was made. */
export interface ProposedProject {
  name: string;
  folder: string;
  /** The lead first. */
  team: { agent: string; lead?: boolean }[];
  /** The first chat's name. */
  chat: string;
  brief: string;
  made?: { project: string; chat: string; message: string };
}

/** An agent the assistant proposed with `propose_agent`, as its block keeps it; once added, as it was added. */
export interface ProposedAgent {
  name: string;
  description: string;
  /** The account's id. */
  account: string;
  /** None is the engine's own default. */
  model: string | null;
  /** The face it is added with. */
  shape: string;
  eyes: string;
  colour: string;
  /** The agent's id, once added. */
  added?: string;
}

/** A project the assistant proposed, and the message that holds it. */
export interface Proposing {
  message: string;
  project: ProposedProject;
}

/** An agent the assistant proposed, and the message that holds it. */
export interface ProposingAgent {
  message: string;
  agent: ProposedAgent;
}

/** Where the Agents screen opens from a card: an agent's page, or the form for a new agent filled in with a proposal. */
export type AgentsAt = { agent: string } | { proposal: ProposingAgent };

/** What a proposal's card needs of the window: the person's agents, Change's form, a chat or the Agents screen to open. */
export const ProposalActs = createContext<{
  agents: Agent[];
  change: (proposing: Proposing) => void;
  open: (chat: string) => void;
  toAgents: (at: AgentsAt) => void;
} | null>(null);

/**
 * A project the assistant proposed, under its words (the owner's pick, 2026-11-22): its name, its folder and its team
 * with their faces, then Make project and Change. Once made, a quiet line in their place opens its first chat.
 */
export function Proposal({ message, project }: Proposing) {
  const acts = useContext(ProposalActs);
  const [making, setMaking] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  const make = () => {
    setMaking(true);
    setFailure(null);
    api.makeProject(message).then(
      () => setMaking(false),
      (f: Failure) => {
        setMaking(false);
        setFailure(f);
      },
    );
  };
  const made = project.made;
  return (
    <div className="proposal">
      <div className="proposal-head">
        <b>{project.name}</b>
        <span>{project.folder}</span>
      </div>
      {project.team.map((seat) => {
        const agent = acts?.agents.find((a) => a.id === seat.agent);
        return (
          <div key={seat.agent} className="proposal-row">
            <Face agent={agent ?? { id: seat.agent, name: 'An agent' }} size={20} />
            <b>{agent?.name ?? 'An agent'}</b>
            {seat.lead && <span className="room-tag">Lead</span>}
            {agent?.description && <span className="proposal-about">{agent.description}</span>}
          </div>
        );
      })}
      <div className="proposal-foot">
        {made ? (
          <span className="room-kept">
            <button type="button" className="room-kept-line" onClick={() => acts?.open(made.chat)}>
              <Folder aria-hidden="true" />
              <span>Made project</span> <b>{project.name}</b>
            </button>
          </span>
        ) : (
          <>
            <Button type="button" disabled={making} onClick={make}>
              Make project
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={making || !acts}
              onClick={() => acts?.change({ message, project })}
            >
              Change
            </Button>
          </>
        )}
      </div>
    </div>
  );
}

/**
 * An agent the assistant proposed, under its words (the owner's pick, 2026-11-22: "it shows the agent first"): its
 * face, name and what it is for, the account and model it would run on, then Add agent and Change. Once added, a quiet
 * line in their place opens its page.
 */
export function AgentProposal({ message, agent }: ProposingAgent) {
  const acts = useContext(ProposalActs);
  const [adding, setAdding] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  const { account, model } = useRunsOn(agent);
  const add = () => {
    setAdding(true);
    setFailure(null);
    api.addProposedAgent(message).then(
      () => setAdding(false),
      (f: Failure) => {
        setAdding(false);
        setFailure(f);
      },
    );
  };
  const added = agent.added;
  return (
    <div className="proposal">
      <div className="proposal-who">
        <Face agent={{ id: added ?? message, ...agent }} size={28} />
        <span>
          <b>{agent.name}</b>
          {agent.description && <span className="proposal-about">{agent.description}</span>}
        </span>
      </div>
      <div className="proposal-set">
        <span>Account</span>
        {account}
        <span>Model</span>
        {model}
      </div>
      <div className="proposal-foot">
        {added ? (
          <span className="room-kept">
            <button type="button" className="room-kept-line" onClick={() => acts?.toAgents({ agent: added })}>
              <AgentsMark />
              <span>Added agent</span> <b>{agent.name}</b>
            </button>
          </span>
        ) : (
          <>
            <Button type="button" disabled={adding} onClick={add}>
              Add agent
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={adding || !acts}
              onClick={() => acts?.toAgents({ proposal: { message, agent } })}
            >
              Change
            </Button>
          </>
        )}
      </div>
    </div>
  );
}

/** The account and model as the Agents screen names them: the engine and the account's name, the model's name. */
function useRunsOn(agent: ProposedAgent) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [models, setModels] = useState<Model[] | null>(null);
  useEffect(() => {
    api.accounts().then(setAccounts, () => {});
  }, []);
  useEffect(() => {
    api.models(agent.account).then(setModels, () => {});
  }, [agent.account]);
  const on = accounts.find((a) => a.id === agent.account);
  return {
    account: on ? `${engineName(on.engine)} · ${on.name}` : '',
    model: models ? chosen(models, agent.model) : '',
  };
}
