import { ChevronLeft, ChevronRight, Folder, Plus } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Agent, api, type Failure, type Project, type ProjectChange, type Role } from '../api';
import { AddedBy } from '../components/AddedBy';
import { danger } from '../components/Confirm';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { DeleteInline } from '../DeleteConfirm';
import { listen } from '../live';
import { report } from '../report';
import { FolderField } from './FolderField';
import { Lead, Team } from './ProjectTeam';
import { Page, Row } from './Row';
import './projects.css';

export type Seat = { agent: string; role: Role };

/**
 * Settings > Projects: the projects listed, each with its folder and team; one opens a page of its own, where its
 * name, folder and team save as they change.
 */
export function Projects({ start, onAdd }: { start?: string; onAdd: () => void }) {
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  const [opened, setOpened] = useState<string | null>(start ?? null);

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
        onBack={() => setOpened(null)}
        onSaved={(done) => setProjects((list) => list?.map((p) => (p.id === done.id ? done : p)) ?? null)}
        onDeleted={(left) => {
          setProjects((list) => list?.filter((p) => p.id !== project.id) ?? null);
          setOpened(null);
          setFailure(left);
        }}
      />
    );
  const face = (id: string, name: string) => agents.find((a) => a.id === id) ?? { id, name };
  return (
    <Page
      title="Projects"
      line="Each project’s folder and default team."
      action={
        // New project's dialog, the one the new chat's project menu opens, over Settings.
        <Button type="button" variant="outline" onClick={onAdd}>
          <Plus />
          Add
        </Button>
      }
    >
      {projects?.length === 0 && <p className="settings-empty">No projects</p>}
      {projects && projects.length > 0 && (
        <ul className="settings-rows settings-top">
          {projects.map((p) => (
            <li key={p.id}>
              <button type="button" className="project-open" onClick={() => setOpened(p.id)}>
                <Row
                  lead={
                    <span className="project-tile" aria-hidden="true">
                      <Folder />
                    </span>
                  }
                  name={p.name}
                  line={
                    <code className="path-tail">
                      <span>{p.folder}</span>
                    </code>
                  }
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

interface PageProps {
  project: Project;
  agents: Agent[];
  onBack: () => void;
  onSaved: (project: Project) => void;
  /** What the delete couldn't remove yet, if anything. */
  onDeleted: (left: Failure | null) => void;
}

type Part = 'name' | 'folder' | 'team';

/** A project's page: each change saves as it is made, as every other Settings row does (the owner's pick 1A, 2026-11-24). */
function ProjectPage({ project, agents, onBack, onSaved, onDeleted }: PageProps) {
  const [deleting, setDeleting] = useState(false);
  const [name, setName] = useState(project.name);
  const [folder, setFolder] = useState(project.folder);
  const [saved, setSaved] = useState<Part | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  const team = project.members.map(({ agent, role }) => ({ agent, role }));

  const save = (part: Part, change: ProjectChange) => {
    setFailure(null);
    setSaved(null);
    api.saveProject(project.id, change).then(
      (done) => {
        onSaved(done);
        setSaved(part);
      },
      (f: Failure) => {
        setFailure(f);
        if (part === 'name') setName(project.name);
        if (part === 'folder') setFolder(project.folder);
      },
    );
  };
  const saveName = (text: string) => {
    const next = text.trim();
    if (!next) setName(project.name);
    else if (next !== project.name) save('name', { name: next });
  };
  const saveFolder = (text: string) => {
    const next = text.trim();
    if (!next) setFolder(project.folder);
    else if (next !== project.folder) save('folder', { folder: next });
  };

  // A name typed and not yet left still saves when the page goes (Escape, another page, Settings closed).
  const unsaved = useRef({ name, project });
  unsaved.current = { name, project };
  useEffect(
    () => () => {
      const { name, project } = unsaved.current;
      const next = name.trim();
      if (next && next !== project.name)
        api
          .saveProject(project.id, { name: next })
          .catch((f: Failure) =>
            report({ name: 'gathr.ui.project.save', error: f.error, attrs: { why: f.why } }),
          );
    },
    [],
  );

  const status = (part: Part) =>
    saved === part ? (
      <span className="muted" role="status">
        Saved
      </span>
    ) : undefined;
  return (
    <Page
      title={project.name}
      line={project.added_by ? <AddedBy app={project.added_by} /> : undefined}
      back={
        <button type="button" className="settings-back" onClick={onBack}>
          <ChevronLeft aria-hidden="true" />
          Projects
        </button>
      }
      action={
        deleting ? (
          <DeleteInline
            kind="project"
            id={project.id}
            name={project.name}
            onCancel={() => setDeleting(false)}
            onDeleted={onDeleted}
          />
        ) : (
          <Button type="button" variant="outline" className={danger} onClick={() => setDeleting(true)}>
            Delete
          </Button>
        )
      }
    >
      <div className="project-form">
        <div className="settings-rows settings-top">
          <Row name={<label htmlFor="project-name">Name</label>} line={status('name')}>
            <input
              id="project-name"
              value={name}
              maxLength={80}
              onChange={(e) => setName(e.target.value)}
              onBlur={(e) => saveName(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && saveName(e.currentTarget.value)}
            />
          </Row>
          <Row name={<label htmlFor="project-folder">Folder</label>} line={status('folder')}>
            <FolderField
              id="project-folder"
              title="Project folder"
              value={folder}
              onChange={setFolder}
              onCommit={saveFolder}
            />
          </Row>
          <Lead seats={team} agents={agents} onChange={(seats) => save('team', { members: seats })} />
        </div>
        <Team
          seats={team}
          agents={agents}
          status={saved === 'team' ? 'Saved' : undefined}
          onChange={(seats) => save('team', { members: seats })}
        />
      </div>
    </Page>
  );
}
