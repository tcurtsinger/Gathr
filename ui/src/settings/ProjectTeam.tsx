import { X } from 'lucide-react';
import { useId } from 'react';
import { SpeakerIdentity } from '@/components/assistant-ui/elements/speaker-identity';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import type { Agent, Role } from '../api';
import { roleName } from '../chat/team';
import type { Seat } from './Projects';

const MOST = 5;
const roles: Role[] = ['supervisor', 'member', 'reviewer'];

interface Props {
  seats: Seat[];
  agents: Agent[];
  onChange: (seats: Seat[]) => void;
}

/**
 * A project's default team (0028): one Supervisor, at most one Reviewer, up to five agents. Picking a
 * Supervisor or Reviewer makes the one before a Member; removing the Supervisor makes the next one it.
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
          <SpeakerIdentity
            className="w-auto"
            turns={[{ id: agent, kind: 'agent', name: name(agent), text: '' }]}
          />
          <select
            aria-label={`Role of ${name(agent)}`}
            value={role}
            onChange={(e) => setRole(agent, e.target.value as Role)}
          >
            {roles.map((r) => (
              <option key={r} value={r}>
                {roleName[r]}
              </option>
            ))}
          </select>
          <TooltipIconButton
            type="button"
            tooltip={`Remove ${name(agent)}`}
            side="left"
            onClick={() => remove(agent)}
          >
            <X />
          </TooltipIconButton>
        </div>
      ))}
      <select
        aria-label="Add agent"
        className="project-add"
        value=""
        disabled={seats.length >= MOST || others.length === 0}
        onChange={(e) => add(e.target.value)}
      >
        <option value="" disabled>
          Add agent
        </option>
        {others.map((a) => (
          <option key={a.id} value={a.id}>
            {a.name}
          </option>
        ))}
      </select>
    </fieldset>
  );
}
