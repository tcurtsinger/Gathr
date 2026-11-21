import { X } from 'lucide-react';
import type { Agent, Role } from '../api';
import { Dropdown } from '../components/Dropdown';
import { Face } from '../components/Face';
import { IconButton } from '../components/IconButton';
import type { Seat } from './Projects';
import { Group, Row } from './Row';

const MOST = 5;
const roles: Role[] = ['supervisor', 'member'];
const roleName: Record<Role, string> = {
  supervisor: 'Lead',
  member: 'Member',
};

interface Props {
  seats: Seat[];
  agents: Agent[];
  onChange: (seats: Seat[]) => void;
}

/**
 * A project's default team (0028): one lead, up to five agents, never none. The lead changes only by
 * picking another, which makes the one before a Member; removing the lead makes the next one it; the last
 * agent can't be removed.
 */
export function Team({ seats, agents, onChange }: Props) {
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
    <>
      <Group
        title="Default team"
        right={<span className="settings-group-line">One lead, up to five agents.</span>}
      />
      <div className="settings-rows settings-top">
        {seats.map(({ agent, role }) => (
          <Row
            key={agent}
            lead={
              <Face
                agent={agents.find((a) => a.id === agent) ?? { id: agent, name: name(agent) }}
                size={24}
              />
            }
            name={name(agent)}
          >
            <Dropdown
              aria-label={`Role of ${name(agent)}`}
              className="project-role"
              disabled={role === 'supervisor'}
              value={role}
              onChange={(v) => setRole(agent, v as Role)}
              choices={roles.map((r) => ({ value: r, label: roleName[r] }))}
            />
            <IconButton
              type="button"
              aria-label={`Remove ${name(agent)}`}
              className="settings-more"
              disabled={seats.length === 1}
              onClick={() => remove(agent)}
            >
              <X />
            </IconButton>
          </Row>
        ))}
        <div className="setting">
          <Dropdown
            aria-label="Add agent"
            className="project-add"
            value=""
            placeholder="Add agent"
            disabled={seats.length >= MOST || others.length === 0}
            onChange={add}
            choices={others.map((a) => ({ value: a.id, label: a.name }))}
          />
        </div>
      </div>
    </>
  );
}
