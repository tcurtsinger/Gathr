import { Folder } from 'lucide-react';
import { useContext, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure } from '../api';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { AgentsMark } from '../Rail';
import { ProposalActs, useRunsOn } from './Proposal';

/** What `propose_agent` sets, as a change's block keeps it before and after. */
export interface AgentSet {
  name: string;
  description: string;
  /** The account's id. */
  account: string;
  /** None is the engine's own default. */
  model: string | null;
  /** Its face; none picked, the window works each out from the agent's id. */
  shape?: string | null;
  eyes?: string | null;
  colour?: string | null;
}

/** A change the assistant proposed to an agent with `propose_agent_change`: the agent then, and as it would be. */
export interface ProposedAgentChange {
  agent: string;
  was: AgentSet;
  to: AgentSet;
  saved?: boolean;
}

/** A project's name, folder and team, the lead first. */
export interface ProjectSet {
  name: string;
  folder: string;
  team: { agent: string; lead?: boolean }[];
}

/** A change the assistant proposed to a project with `propose_project_change`. */
export interface ProposedProjectChange {
  project: string;
  was: ProjectSet;
  to: ProjectSet;
  saved?: boolean;
}

/** Save, from the card: what it is doing and what went wrong, for a card in the corner. */
function useSave(message: string) {
  const [saving, setSaving] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  const save = () => {
    setSaving(true);
    setFailure(null);
    api.saveProposedChange(message).then(
      () => setSaving(false),
      (f: Failure) => {
        setSaving(false);
        setFailure(f);
      },
    );
  };
  return { saving, save };
}

/** One changed value: the old one struck through, an arrow, the new one. */
function Changed({ was, now }: { was: string; now: string }) {
  return (
    <span className="proposal-change">
      <s>{was}</s>
      <span aria-hidden="true">→</span>
      <b>{now}</b>
    </span>
  );
}

/**
 * A change the assistant proposed to an agent (the owner's pick, 2026-11-24, A: the changes only): the agent as it is,
 * then each field that changes, old to new, then Save and Change, which opens its page. Once saved, a quiet line.
 */
export function AgentChangeCard({ message, change }: { message: string; change: ProposedAgentChange }) {
  const acts = useContext(ProposalActs);
  const { saving, save } = useSave(message);
  const { was, to } = change;
  const before = useRunsOn(was);
  const after = useRunsOn(to);
  const face = (set: AgentSet) => ({
    id: change.agent,
    name: set.name,
    shape: set.shape,
    eyes: set.eyes,
    colour: set.colour,
  });
  const look = ['shape', 'eyes', 'colour'] as const;
  const restyled = look.some((k) => (was[k] ?? null) !== (to[k] ?? null));
  const rows: [string, string, string][] = [
    ['Name', was.name, to.name],
    ['About', was.description, to.description],
    ['Account', before.account, after.account],
    ['Model', before.model, after.model],
  ];
  const moved = was.account !== to.account;
  const differs: Record<string, boolean> = {
    Name: was.name !== to.name,
    About: was.description !== to.description,
    Account: moved,
    Model: was.model !== to.model || (moved && before.model !== after.model),
  };
  const shown = rows.filter(([field]) => differs[field]);
  return (
    <div className="proposal">
      <div className="proposal-who">
        <Face agent={face(change.saved ? to : was)} size={28} />
        <span>
          <b>{change.saved ? to.name : was.name}</b>
          {was.description && <span className="proposal-about">{was.description}</span>}
        </span>
      </div>
      <div className="proposal-set">
        {shown.map(([field, from, now]) => [
          <span key={field}>{field}</span>,
          <Changed key={`${field}-v`} was={from} now={now} />,
        ])}
        {restyled && (
          <>
            <span>Look</span>
            <span className="proposal-change">
              <Face agent={face(was)} size={20} />
              <span aria-hidden="true">→</span>
              <Face agent={face(to)} size={20} />
            </span>
          </>
        )}
      </div>
      <div className="proposal-foot">
        {change.saved ? (
          <span className="room-kept">
            <button
              type="button"
              className="room-kept-line"
              onClick={() => acts?.toAgents({ agent: change.agent })}
            >
              <AgentsMark />
              <span>Changed agent</span> <b>{to.name}</b>
            </button>
          </span>
        ) : (
          <>
            <Button type="button" disabled={saving} onClick={save}>
              Save
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={saving || !acts}
              onClick={() => acts?.toAgents({ agent: change.agent })}
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
 * A change the assistant proposed to a project (the owner's pick, 2026-11-24, A): the project as it is, its name and
 * folder old to new, then, when the team changes, the new team with who joins and the ones who leave, then Save and
 * Change, which opens its settings. Once saved, a quiet line.
 */
export function ProjectChangeCard({ message, change }: { message: string; change: ProposedProjectChange }) {
  const acts = useContext(ProposalActs);
  const { saving, save } = useSave(message);
  const { was, to } = change;
  const fields: [string, string, string][] = [
    ['Name', was.name, to.name],
    ['Folder', was.folder, to.folder],
  ];
  const shown = fields.filter(([, from, now]) => from !== now);
  const key = (team: ProjectSet['team']) => team.map((s) => `${s.agent}:${!!s.lead}`).join();
  const team = key(was.team) !== key(to.team);
  const had = (agent: string) => was.team.some((s) => s.agent === agent);
  const leaving = was.team.filter((s) => !to.team.some((t) => t.agent === s.agent));
  const seat = (s: ProjectSet['team'][number], tag?: string) => {
    const agent = acts?.agents.find((a) => a.id === s.agent);
    return (
      <div key={s.agent} className={tag === 'Leaves' ? 'proposal-row proposal-leaves' : 'proposal-row'}>
        <Face agent={agent ?? { id: s.agent, name: 'An agent' }} size={20} />
        <b>{agent?.name ?? 'An agent'}</b>
        {s.lead && tag !== 'Leaves' && <span className="room-tag">Lead</span>}
        {tag && <span className="room-tag">{tag}</span>}
        {agent?.description && <span className="proposal-about">{agent.description}</span>}
      </div>
    );
  };
  return (
    <div className="proposal">
      <div className="proposal-head">
        <b>{change.saved ? to.name : was.name}</b>
        <span>{change.saved ? to.folder : was.folder}</span>
      </div>
      {shown.length > 0 && (
        <div className="proposal-set">
          {shown.map(([field, from, now]) => [
            <span key={field}>{field}</span>,
            <Changed key={`${field}-v`} was={from} now={now} />,
          ])}
        </div>
      )}
      {team && (
        <>
          <div className="proposal-set proposal-team">
            <span>Team</span>
          </div>
          {to.team.map((s) => seat(s, had(s.agent) ? undefined : 'Joins'))}
          {leaving.map((s) => seat(s, 'Leaves'))}
        </>
      )}
      <div className="proposal-foot">
        {change.saved ? (
          <span className="room-kept">
            <button type="button" className="room-kept-line" onClick={() => acts?.toProject(change.project)}>
              <Folder aria-hidden="true" />
              <span>Changed project</span> <b>{to.name}</b>
            </button>
          </span>
        ) : (
          <>
            <Button type="button" disabled={saving} onClick={save}>
              Save
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={saving || !acts}
              onClick={() => acts?.toProject(change.project)}
            >
              Change
            </Button>
          </>
        )}
      </div>
    </div>
  );
}
