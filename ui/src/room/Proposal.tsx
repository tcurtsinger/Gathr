import { Folder } from 'lucide-react';
import { createContext, useContext, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Agent, api, type Failure, type ProposedProject } from '../api';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import './proposal.css';

/** A project the assistant proposed, and the message that holds it. */
export interface Proposing {
  message: string;
  project: ProposedProject;
}

/** What a proposal's card needs of the window: the person's agents, Change's form, and a chat to open. */
export const ProposalActs = createContext<{
  agents: Agent[];
  change: (proposing: Proposing) => void;
  open: (chat: string) => void;
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
