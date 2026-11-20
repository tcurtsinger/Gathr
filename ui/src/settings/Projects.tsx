import { useCallback, useEffect, useId, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Agent, api, type Failure, type Project, type ProjectChange, type Role } from '../api';
import { danger } from '../components/Confirm';
import { FailureNotice } from '../components/Notice';
import { DeleteConfirm } from '../DeleteConfirm';
import { listen } from '../live';
import { Team } from './ProjectTeam';
import './agents.css';
import './projects.css';

type Open = (ref: string) => void;
export type Seat = { agent: string; role: Role };

/**
 * Settings > Projects (0043, the owner's pick B): the projects listed as Agents lists agents, the picked
 * one's name, folder and team, in one draft with one Save.
 */
export function Projects({ start, onReference }: { start?: string; onReference: Open }) {
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [selected, setSelected] = useState<string | null>(start ?? null);
  /** The project last saved, so its form says so until it changes again. */
  const [saved, setSaved] = useState<string | null>(null);

  const load = useCallback(() => {
    api.projects().then(setProjects, setFailure);
    api.agents().then((a) => setAgents(a.agents), setFailure);
  }, []);
  useEffect(() => {
    load();
    return listen((event) => {
      if (event.type === 'project' || event.type === 'agents' || event.type === 'lagged') load();
    });
  }, [load]);

  const shown = selected ?? projects?.[0]?.id ?? null;
  const project = projects?.find((p) => p.id === shown);
  return (
    <section className="agents" aria-label="Projects">
      <div className="agent-list">
        {projects?.map((p) => (
          <button
            type="button"
            key={p.id}
            aria-current={p.id === shown ? 'true' : undefined}
            onClick={() => {
              setSelected(p.id);
              setSaved(null);
            }}
          >
            {p.name}
          </button>
        ))}
      </div>
      <div className="agent-form">
        {failure && <FailureNotice failure={failure} onReference={onReference} />}
        {projects?.length === 0 && <p className="muted">No projects</p>}
        {project && (
          // A new form for each project, so one project's draft never carries to another.
          <ProjectForm
            key={project.id}
            project={project}
            agents={agents}
            saved={saved === project.id}
            onSaved={(done) => {
              setProjects((list) => list?.map((p) => (p.id === done.id ? done : p)) ?? null);
              setSaved(done.id);
            }}
            onDeleted={(left) => {
              setProjects((list) => list?.filter((p) => p.id !== project.id) ?? null);
              setSelected(null);
              setFailure(left);
            }}
            onReference={onReference}
          />
        )}
      </div>
    </section>
  );
}

type Fields = Pick<Project, 'name' | 'folder'> & { team: Seat[] };

interface FormProps {
  project: Project;
  agents: Agent[];
  saved: boolean;
  onSaved: (project: Project) => void;
  /** What the delete couldn't remove yet, if anything. */
  onDeleted: (left: Failure | null) => void;
  onReference: Open;
}

function ProjectForm({ project, agents, saved, onSaved, onDeleted, onReference }: FormProps) {
  /** Delete's button while its confirm is open: focus goes back to it on Cancel. */
  const [deleting, setDeleting] = useState<HTMLElement | null>(null);
  // Only the fields edited here, over the project as saved, so Save never writes an untouched one back.
  const [edits, setEdits] = useState<Partial<Fields>>({});
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const title = useId();
  const team = project.members.map(({ agent, role }) => ({ agent, role }));
  const fields: Fields = { ...project, team, ...edits };
  const edit = (more: Partial<Fields>) => setEdits((e) => ({ ...e, ...more }));

  const change: ProjectChange = {};
  if (fields.name.trim() !== project.name) change.name = fields.name.trim();
  if (fields.folder.trim() !== project.folder) change.folder = fields.folder.trim();
  if (JSON.stringify(fields.team) !== JSON.stringify(team)) change.members = fields.team;
  const changed = Object.keys(change).length > 0;

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setFailure(null);
    api.saveProject(project.id, change).then(
      (done) => {
        setBusy(false);
        setEdits({});
        onSaved(done);
      },
      (f: Failure) => {
        setBusy(false);
        setFailure(f);
      },
    );
  };
  return (
    <form className="project-form" aria-labelledby={title} onSubmit={submit}>
      <h3 id={title}>{project.name}</h3>
      <label className="agent-field">
        Name
        <input value={fields.name} maxLength={80} onChange={(e) => edit({ name: e.target.value })} />
      </label>
      <label className="agent-field">
        Folder
        <input
          className="project-folder"
          value={fields.folder}
          spellCheck={false}
          onChange={(e) => edit({ folder: e.target.value })}
        />
      </label>
      <Team seats={fields.team} agents={agents} onChange={(seats) => edit({ team: seats })} />
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
      <div className="agent-actions">
        <Button
          type="button"
          variant="outline"
          className={danger}
          disabled={busy}
          onClick={(e) => setDeleting(e.currentTarget)}
        >
          Delete
        </Button>
        {deleting && (
          <DeleteConfirm
            kind="project"
            id={project.id}
            name={project.name}
            opener={deleting}
            onCancel={() => setDeleting(null)}
            onDeleted={onDeleted}
            onReference={onReference}
          />
        )}
        <span className="muted" role="status">
          {saved && !changed ? 'Saved' : ''}
        </span>
        <Button type="button" variant="ghost" onClick={() => setEdits({})} disabled={busy || !changed}>
          Cancel
        </Button>
        <Button type="submit" disabled={busy || !changed || !fields.name.trim() || !fields.folder.trim()}>
          Save
        </Button>
      </div>
    </form>
  );
}
