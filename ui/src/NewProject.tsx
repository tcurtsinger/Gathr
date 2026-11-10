import { type FormEvent, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Agent, api, type Failure, type Project } from './api';
import { Dialog } from './components/Dialog';
import { Notice } from './components/Notice';
import { Switch } from './components/Switch';
import './new-project.css';

const MOST = 5;

interface Props {
  open: boolean;
  agents: Agent[];
  projects: Project[];
  onClose: () => void;
  onCreated: (project: Project) => void;
  onReference: (ref: string) => void;
}

/** A project: its name, its folder and its team of up to five, one of them the Supervisor (0028). */
export function NewProject({ open, agents, projects, onClose, onCreated, onReference }: Props) {
  const [name, setName] = useState('');
  const [folder, setFolder] = useState('');
  const [team, setTeam] = useState<string[]>([]);
  const [lead, setLead] = useState('');
  const [reviewer, setReviewer] = useState('');
  const [failure, setFailure] = useState<Failure | null>(null);
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
            <label className="agent-field">
              Supervisor
              <select value={supervisor} onChange={(e) => setLead(e.target.value)}>
                {team.map((id) => (
                  <option key={id} value={id}>
                    {nameOf(id)}
                  </option>
                ))}
              </select>
            </label>
            <label className="agent-field">
              Reviewer
              <select value={reviewer} onChange={(e) => setReviewer(e.target.value)}>
                <option value="">None</option>
                {team
                  .filter((id) => id !== supervisor)
                  .map((id) => (
                    <option key={id} value={id}>
                      {nameOf(id)}
                    </option>
                  ))}
              </select>
            </label>
          </>
        )}
        {failure && (
          <Notice
            danger
            title={failure.error}
            reference={
              failure.ref ? { ref: failure.ref, open: () => onReference(failure.ref as string) } : undefined
            }
            links={
              failure.why && (
                <span className="muted new-project-why" title={failure.why}>
                  {failure.why}
                </span>
              )
            }
          />
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
