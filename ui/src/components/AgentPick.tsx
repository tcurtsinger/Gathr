import { Dropdown } from './Dropdown';
import { Face } from './Face';
import './project-pick.css';

/** Who a new chat's first message goes to, picked beside its project. */
export function AgentPick({
  agent,
  agents,
  onAgent,
}: {
  agent: string;
  agents: readonly { id: string; name: string }[];
  onAgent: (agent: string) => void;
}) {
  return (
    <div className="project-pick">
      <Dropdown
        aria-label="Agent"
        side="top"
        value={agent}
        onChange={onAgent}
        choices={agents.map((a) => ({ value: a.id, label: a.name, icon: <Face agent={a} size={16} /> }))}
      />
    </div>
  );
}
