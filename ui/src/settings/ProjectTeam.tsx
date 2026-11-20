import { X } from 'lucide-react';
import { useId } from 'react';
import type { Agent, Role } from '../api';
import { Dropdown } from '../components/Dropdown';
import { Face } from '../components/Face';
import { IconButton } from '../components/IconButton';
import type { Seat } from './Projects';

const MOST = 5;
const roles: Role[] = ['supervisor', 'member', 'reviewer'];
const roleName: Record<Role, string> = {
  supervisor: 'Supervisor',
  reviewer: 'Reviewer',
  member: 'Member',
};

interface Props {
  seats: Seat[];
  agents: Agent[];
  onChange: (seats: Seat[]) => void;
}

/**
 * A project's default team (0028): one Supervisor, at most one Reviewer, up to five agents, never none.
 * The Supervisor changes only by picking another, which makes the one before a Member, as a new Reviewer
 * does; removing the Supervisor makes the next one it; the last agent can't be removed.
 */
export function Team({ seats, agents, onChange }: Props) {
  const heading = useId();
  const name = (id: string) => agents.find((a) => a.id === id)?.name ?? 'An agent';
  const setRole = (agent: string, role: Role) =>
    onChange(
      seats.map((s) =>
        s.agent === agent
          ? { agent, role }
          : role !== 'member' && s.role === role
            ? { ...s, role: 'member' }
            : s,
      ),
    );
  const remove = (agent: string) => {
    const left = seats.filter((s) => s.agent !== agent);
    const lead = left.some((s) => s.role === 'supervisor');
    onChange(lead ? left : left.map((s, i) => (i === 0 ? { ...s, role: 'supervisor' } : s)));
  };
  const add = (agent: string) =>
    onChange([...seats, { agent, role: seats.length === 0 ? 'supervisor' : 'member' }]);
  const others = agents.filter((a) => !seats.some((s) => s.agent === a.id));

  return (
    <fieldset className="agent-field project-team" aria-labelledby={heading}>
      <h4 id={heading}>Default team</h4>
      <p className="muted">One Supervisor, up to five agents.</p>
      {seats.map(({ agent, role }) => (
        <div key={agent} className="project-seat">
          <span className="project-seat-who">
            <Face agent={agents.find((a) => a.id === agent) ?? { id: agent, name: name(agent) }} size={24} />
            {name(agent)}
          </span>
          <Dropdown
            aria-label={`Role of ${name(agent)}`}
            disabled={role === 'supervisor'}
            value={role}
            onChange={(v) => setRole(agent, v as Role)}
            choices={roles.map((r) => ({ value: r, label: roleName[r] }))}
          />
          <IconButton
            type="button"
            aria-label={`Remove ${name(agent)}`}
            disabled={seats.length === 1}
            onClick={() => remove(agent)}
          >
            <X />
          </IconButton>
        </div>
      ))}
      <Dropdown
        aria-label="Add agent"
        className="project-add"
        value=""
        placeholder="Add agent"
        disabled={seats.length >= MOST || others.length === 0}
        onChange={add}
        choices={others.map((a) => ({ value: a.id, label: a.name }))}
      />
    </fieldset>
  );
}
