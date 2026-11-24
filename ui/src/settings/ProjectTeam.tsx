import { Plus, X } from 'lucide-react';
import type { ReactNode } from 'react';
import type { Agent, Role } from '../api';
import { Dropdown } from '../components/Dropdown';
import { Face } from '../components/Face';
import { IconButton } from '../components/IconButton';
import type { Seat } from './Projects';
import { Group, Row } from './Row';

const MOST = 5;

interface Props {
  seats: Seat[];
  agents: Agent[];
  onChange: (seats: Seat[]) => void;
  /** At the group's right in place of the rule: "Saved". */
  status?: ReactNode;
}

/** A project's lead, picked from its team (the owner's pick 6B, 2026-11-24): a row in the list with its name and folder. */
export function Lead({ seats, agents, onChange }: Omit<Props, 'status'>) {
  if (seats.length === 0) return null;
  const agent = (id: string) => agents.find((a) => a.id === id) ?? { id, name: 'An agent' };
  const lead = (seats.find((s) => s.role === 'supervisor') ?? seats[0]).agent;
  const setLead = (id: string) =>
    onChange(
      seats.map((s) => ({ agent: s.agent, role: (s.agent === id ? 'supervisor' : 'member') as Role })),
    );
  return (
    <Row name={<span id="project-lead">Lead</span>}>
      <Dropdown
        aria-labelledby="project-lead"
        className="project-lead"
        value={lead}
        onChange={setLead}
        choices={seats.map((s) => ({
          value: s.agent,
          label: agent(s.agent).name,
          icon: <Face agent={agent(s.agent)} size={16} />,
        }))}
      />
    </Row>
  );
}

/**
 * A project's default team (0028): one lead, up to five agents, never none. The lead is picked above the list (`Lead`);
 * removing the lead makes the next one it; the last agent can't be removed.
 */
export function Team({ seats, agents, onChange, status }: Props) {
  const agent = (id: string) => agents.find((a) => a.id === id) ?? { id, name: 'An agent' };
  const remove = (id: string) => {
    const left = seats.filter((s) => s.agent !== id);
    const led = left.some((s) => s.role === 'supervisor');
    onChange(led ? left : left.map((s, i) => (i === 0 ? { ...s, role: 'supervisor' } : s)));
  };
  const add = (id: string) =>
    onChange([...seats, { agent: id, role: seats.length === 0 ? 'supervisor' : 'member' }]);
  const others = agents.filter((a) => !seats.some((s) => s.agent === a.id));

  return (
    <>
      <Group
        title="Default team"
        right={
          <span className="settings-group-line" role={status ? 'status' : undefined}>
            {status ?? 'Up to five agents.'}
          </span>
        }
      />
      <div className="settings-rows settings-top">
        {seats.map((s) => (
          <Row key={s.agent} lead={<Face agent={agent(s.agent)} size={24} />} name={agent(s.agent).name}>
            <IconButton
              type="button"
              aria-label={`Remove ${agent(s.agent).name}`}
              className="settings-more"
              disabled={seats.length === 1}
              onClick={() => remove(s.agent)}
            >
              <X />
            </IconButton>
          </Row>
        ))}
        {/* An outlined Add, as at the top of Engines and Projects, opening the agents not on the team; none when nobody can be added. */}
        {seats.length < MOST && others.length > 0 && (
          <div className="setting">
            <Dropdown
              aria-label="Add agent"
              className="project-add"
              value=""
              shown={
                <>
                  <Plus aria-hidden="true" />
                  Add agent
                </>
              }
              onChange={add}
              choices={others.map((a) => ({ value: a.id, label: a.name }))}
            />
          </div>
        )}
      </div>
    </>
  );
}
