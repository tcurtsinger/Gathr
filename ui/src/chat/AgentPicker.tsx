import type { Agent } from '../api';

const EDIT = 'edit-agent';

interface Props {
  agents: Agent[];
  selected: string;
  onSelect: (agent: string) => void;
  /** Opens Settings > Agents at the selected agent. */
  onEdit: () => void;
}

/** Who the next message goes to. It never changes the agent's model or effort (0026). */
export function AgentPicker({ agents, selected, onSelect, onEdit }: Props) {
  return (
    <select
      className="agent-picker"
      aria-label="Agent"
      value={selected}
      onChange={(e) => (e.target.value === EDIT ? onEdit() : onSelect(e.target.value))}
    >
      {agents.map((a) => (
        <option key={a.id} value={a.id}>
          {a.name}
        </option>
      ))}
      <option value={EDIT}>Edit agent</option>
    </select>
  );
}
