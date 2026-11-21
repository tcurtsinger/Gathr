import { type FormEvent, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Agent, api, type Failure, type Project } from './api';
import { useCard } from './components/Corner';
import { Dialog } from './components/Dialog';
import { Dropdown } from './components/Dropdown';
import { Switch } from './components/Switch';
import './new-project.css';

const MOST = 5;

interface Props {
  open: boolean;
  agents: Agent[];
  projects: Project[];
  onClose: () => void;
  onCreated: (project: Project) => void;
}

/** A project: its name, its folder and its team of up to five, one of them the Supervisor (0028). */
export function NewProject({ open, agents, projects, onClose, onCreated }: Props) {
  const [name, setName] = useState('');
  const [folder, setFolder] = useState('');
  const [team, setTeam] = useState<string[]>([]);
  const [lead, setLead] = useState('');
  const [reviewer, setReviewer] = useState('');
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(open ? failure : null);
  const supervisor = team.includes(lead) ? lead : team[0];
  const taken = projects.some((p) => p.name.toLowerCase() === name.trim().toLowerCase());
  const ready = name.trim() && folder.trim() && team.length > 0 && !taken;
  const nameOf = (id: string) => agents.find((a) => a.id === id)?.name ?? id;

  const create = (e: FormEvent) => {
    e.preventDefault();
    if (!ready) return;
    const roles = team.map((agent) => ({
      agent,
      role:
        agent === supervisor
          ? ('supervisor' as const)
          : agent === reviewer
            ? ('reviewer' as const)
            : ('member' as const),
    }));
    api.addProject({ name: name.trim(), folder: folder.trim(), members: roles }).then((project) => {
      setName('');
      setFolder('');
      setTeam([]);
      setFailure(null);
      onCreated(project);
    }, setFailure);
  };

  return (
    <Dialog open={open} title="New project" onClose={onClose} className="new-project">
      <form onSubmit={create}>
        <label className="agent-field">
          Name
          <input value={name} onChange={(e) => setName(e.target.value)} required />
        </label>
        {taken && <p className="muted">Name already used</p>}
        <label className="agent-field">
          Folder
          <input value={folder} onChange={(e) => setFolder(e.target.value)} required spellCheck={false} />
        </label>
        <fieldset className="agent-field">
          <legend>
            Agents{' '}
            <small>
              {team.length} of {MOST}
            </small>
          </legend>
          {agents.map((a) => {
            const on = team.includes(a.id);
            return (
              <div key={a.id} className="new-project-agent">
                <span id={`project-agent-${a.id}`}>{a.name}</span>
                <Switch
                  on={on}
                  labelledBy={`project-agent-${a.id}`}
                  disabled={!on && team.length >= MOST}
                  onChange={() => setTeam(on ? team.filter((id) => id !== a.id) : [...team, a.id])}
                />
              </div>
            );
          })}
        </fieldset>
        {team.length > 1 && (
          <>
            <div className="agent-field">
              <label htmlFor="new-project-supervisor">Supervisor</label>
              <Dropdown
                id="new-project-supervisor"
                value={supervisor}
                onChange={setLead}
                choices={team.map((id) => ({ value: id, label: nameOf(id) }))}
              />
            </div>
            <div className="agent-field">
              <label htmlFor="new-project-reviewer">Reviewer</label>
              <Dropdown
                id="new-project-reviewer"
                value={reviewer}
                onChange={setReviewer}
                choices={[
                  { value: '', label: 'None' },
                  ...team.filter((id) => id !== supervisor).map((id) => ({ value: id, label: nameOf(id) })),
                ]}
              />
            </div>
          </>
        )}
        <footer className="agent-actions">
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" disabled={!ready}>
            Create
          </Button>
        </footer>
      </form>
    </Dialog>
  );
}
