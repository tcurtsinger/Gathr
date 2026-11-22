import { ChevronLeft, ChevronRight, Folder } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Agent, api, type Failure, type Project, type ProjectChange, type Role } from '../api';
import { danger } from '../components/Confirm';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { DeleteConfirm } from '../DeleteConfirm';
import { listen } from '../live';
import { Team } from './ProjectTeam';
import { Page, Row } from './Row';
import './projects.css';

export type Seat = { agent: string; role: Role };

/**
 * Settings > Projects: the projects listed, each with its folder and team; one opens a page of its own, its
 * name, folder and team in one draft with one Save.
 */
export function Projects({ start }: { start?: string }) {
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  const [opened, setOpened] = useState<string | null>(start ?? null);
  /** The project last saved, so its page says so until it changes again. */
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

  const project = projects?.find((p) => p.id === opened);
  if (project)
    return (
      // A new form for each project, so one project's draft never carries to another.
      <ProjectPage
        key={project.id}
        project={project}
        agents={agents}
        saved={saved === project.id}
        onBack={() => setOpened(null)}
        onSaved={(done) => {
          setProjects((list) => list?.map((p) => (p.id === done.id ? done : p)) ?? null);
          setSaved(done.id);
        }}
        onDeleted={(left) => {
          setProjects((list) => list?.filter((p) => p.id !== project.id) ?? null);
          setOpened(null);
          setFailure(left);
        }}
      />
    );
  const face = (id: string, name: string) => agents.find((a) => a.id === id) ?? { id, name };
  return (
    <Page title="Projects" line="Each project’s folder and default team.">
      {projects?.length === 0 && <p className="settings-empty">No projects</p>}
      {projects && projects.length > 0 && (
        <ul className="settings-rows settings-top">
          {projects.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                className="project-open"
                onClick={() => {
                  setOpened(p.id);
                  setSaved(null);
                }}
              >
                <Row
                  lead={
                    <span className="project-tile" aria-hidden="true">
                      <Folder />
                    </span>
                  }
                  name={p.name}
                  line={<code>{p.folder}</code>}
                >
                  <span className="project-faces">
                    {p.members.map((m) => (
                      <Face key={m.agent} agent={face(m.agent, m.name)} size={24} />
                    ))}
                  </span>
                  <ChevronRight className="project-chevron" aria-hidden="true" />
                </Row>
              </button>
            </li>
          ))}
        </ul>
      )}
    </Page>
  );
}

type Fields = Pick<Project, 'name' | 'folder'> & { team: Seat[] };

interface PageProps {
  project: Project;
  agents: Agent[];
  saved: boolean;
  onBack: () => void;
  onSaved: (project: Project) => void;
  /** What the delete couldn't remove yet, if anything. */
  onDeleted: (left: Failure | null) => void;
}

function ProjectPage({ project, agents, saved, onBack, onSaved, onDeleted }: PageProps) {
  /** Delete's button while its confirm is open: focus goes back to it on Cancel. */
  const [deleting, setDeleting] = useState<HTMLElement | null>(null);
  // Only the fields edited here, over the project as saved, so Save never writes an untouched one back.
  const [edits, setEdits] = useState<Partial<Fields>>({});
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
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
    <Page
      title={project.name}
      back={
        <button type="button" className="settings-back" onClick={onBack}>
          <ChevronLeft aria-hidden="true" />
          Projects
        </button>
      }
      action={
        <Button
          type="button"
          variant="outline"
          className={danger}
          disabled={busy}
          onClick={(e) => setDeleting(e.currentTarget)}
        >
          Delete
        </Button>
      }
    >
      {deleting && (
        <DeleteConfirm
          kind="project"
          id={project.id}
          name={project.name}
          opener={deleting}
          onCancel={() => setDeleting(null)}
          onDeleted={onDeleted}
        />
      )}
      <form className="project-form" aria-label={project.name} onSubmit={submit}>
        <div className="settings-rows settings-top">
          <Row name={<label htmlFor="project-name">Name</label>}>
            <input
              id="project-name"
              value={fields.name}
              maxLength={80}
              onChange={(e) => edit({ name: e.target.value })}
            />
          </Row>
          <Row name={<label htmlFor="project-folder">Folder</label>}>
            <input
              id="project-folder"
              className="project-folder"
              value={fields.folder}
              spellCheck={false}
              onChange={(e) => edit({ folder: e.target.value })}
            />
          </Row>
        </div>
        <Team seats={fields.team} agents={agents} onChange={(seats) => edit({ team: seats })} />
        <footer className="settings-foot">
          <span className="muted" role="status">
            {saved && !changed ? 'Saved' : ''}
          </span>
          <Button type="button" variant="outline" onClick={() => setEdits({})} disabled={busy || !changed}>
            Cancel
          </Button>
          <Button
            type="submit"
            variant="outline"
            disabled={busy || !changed || !fields.name.trim() || !fields.folder.trim()}
          >
            Save
          </Button>
        </footer>
      </form>
    </Page>
  );
}
