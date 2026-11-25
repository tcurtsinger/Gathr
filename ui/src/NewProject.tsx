import { type FormEvent, useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Agent, api, type Failure, type Project } from './api';
import { useCard } from './components/Corner';
import { Dialog } from './components/Dialog';
import type { Proposing } from './room/Proposal';
import { FolderField } from './settings/FolderField';
import type { Seat } from './settings/Projects';
import { Lead, Team } from './settings/ProjectTeam';
import { Row } from './settings/Row';
import './new-project.css';

interface Props {
  open: boolean;
  agents: Agent[];
  projects: Project[];
  /** Change, on a project the assistant proposed: the form opens filled in with it, and Create makes it from there. */
  proposal?: Proposing | null;
  onClose: () => void;
  onCreated: (project: Project) => void;
}

/**
 * A project: its name, its folder and its team of up to five, one of them the lead (0028), in the rows and team
 * editor of Settings > Projects.
 */
export function NewProject({ open, agents, projects, proposal, onClose, onCreated }: Props) {
  const [name, setName] = useState('');
  const [folder, setFolder] = useState('');
  const [seats, setSeats] = useState<Seat[]>([]);
  const [failure, setFailure] = useState<Failure | null>(null);
  // What a proposal filled in, and cleared again once its form closes.
  const [filled, setFilled] = useState<Proposing | null>(null);
  if ((proposal ?? null) !== filled) {
    const p = proposal?.project;
    setFilled(proposal ?? null);
    setName(p?.name ?? '');
    setFolder(p?.folder ?? '');
    const lead = p?.team.find((s) => s.lead)?.agent ?? p?.team[0]?.agent;
    setSeats(
      p?.team.map((s) => ({ agent: s.agent, role: s.agent === lead ? 'supervisor' : 'member' })) ?? [],
    );
    setFailure(null);
  }
  // A folder that's there but not in a git repository: Create sets git up first (0028 needs one for worktrees).
  const [bare, setBare] = useState<string | null>(null);
  useEffect(() => {
    const path = folder.trim();
    if (!open || !path) return;
    let live = true;
    const timer = setTimeout(() => {
      api.checkFolder(path).then(
        (c) => live && setBare(c.folder && !c.repo ? path : null),
        () => live && setBare(null),
      );
    }, 300);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [open, folder]);
  const init = bare !== null && bare === folder.trim();
  useCard(open ? failure : null);
  const taken = projects.some((p) => p.name.toLowerCase() === name.trim().toLowerCase());
  const ready = name.trim() && folder.trim() && seats.length > 0 && !taken;

  const create = (e: FormEvent) => {
    e.preventDefault();
    if (!ready) return;
    const asked = { name: name.trim(), folder: folder.trim(), members: seats, ...(init && { init: true }) };
    if (proposal) {
      api.makeProject(proposal.message, asked).then(onClose, setFailure);
      return;
    }
    api.addProject(asked).then((project) => {
      setName('');
      setFolder('');
      setSeats([]);
      setFailure(null);
      onCreated(project);
    }, setFailure);
  };

  return (
    <Dialog open={open} title="New project" onClose={onClose} className="new-project" noHeader closeIcon>
      <form className="settings-col" onSubmit={create}>
        <header className="settings-head">
          <h3>New project</h3>
        </header>
        <div className="settings-rows settings-top">
          <Row
            name={<label htmlFor="new-project-name">Name</label>}
            line={taken ? 'Name already used' : undefined}
          >
            <input
              id="new-project-name"
              value={name}
              maxLength={80}
              placeholder="Website"
              onChange={(e) => setName(e.target.value)}
              required
            />
          </Row>
          <Row
            name={<label htmlFor="new-project-folder">Folder</label>}
            line={init ? 'Not a git repo' : undefined}
          >
            <FolderField id="new-project-folder" title="Project folder" value={folder} onChange={setFolder} />
          </Row>
          <Lead seats={seats} agents={agents} onChange={setSeats} />
        </div>
        <Team seats={seats} agents={agents} onChange={setSeats} />
        <footer className="settings-foot">
          <Button type="button" variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" disabled={!ready}>
            {init ? 'Set up git and create' : 'Create'}
          </Button>
        </footer>
      </form>
    </Dialog>
  );
}
