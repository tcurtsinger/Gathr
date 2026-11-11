import { useCallback, useEffect, useId, useState } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { type Agent, api, type Failure, type Project, type ProjectChange, type Role } from '../api';
import { began } from '../chat/waits';
import { FailureNotice } from '../components/Notice';
import { listen } from '../live';
import { Team } from './ProjectTeam';
import './agents.css';
import './projects.css';

type Open = (ref: string) => void;
export type Seat = { agent: string; role: Role };

/**
 * Settings > Projects (0036, the owner's pick B): the projects listed as Agents lists agents, the picked
 * one's General (name, folder, team) and Checks & merge (its checks read only, the CI choice and merge
 * method), in one draft with one Save.
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
    api.agents().then(setAgents, setFailure);
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
            onReference={onReference}
          />
        )}
      </div>
    </section>
  );
}

type Fields = Pick<Project, 'name' | 'folder' | 'ci' | 'merge'> & { team: Seat[] };

const required: [Project['ci'], string, string][] = [
  ['ci', 'CI', 'CI must pass on the reviewed commit.'],
  ['gathr', 'Gathr’s checks', 'Gathr’s checks must pass on the reviewed commit.'],
  ['both', 'Both', 'CI and Gathr’s checks must pass on the reviewed commit.'],
];
const methods: [Project['merge'], string, string][] = [
  ['fast-forward', 'Fast-forward (default)', 'Keeps linear history and pins the checked base.'],
  ['merge', 'Merge commit', 'Creates a merge commit with the checked base.'],
  ['squash', 'Squash via GitHub API', 'Pins the reviewed head; GitHub chooses the base at merge time.'],
];

interface FormProps {
  project: Project;
  agents: Agent[];
  saved: boolean;
  onSaved: (project: Project) => void;
  onReference: Open;
}

function ProjectForm({ project, agents, saved, onSaved, onReference }: FormProps) {
  const [section, setSection] = useState<'general' | 'checks'>('general');
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
  if (fields.merge !== project.merge) change.merge = fields.merge;
  if (fields.ci !== project.ci) change.ci = fields.ci;
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
  const tab = (id: typeof section, label: string) => (
    <button type="button" aria-current={section === id ? 'true' : undefined} onClick={() => setSection(id)}>
      {label}
    </button>
  );

  return (
    <form className="project-form" aria-labelledby={title} onSubmit={submit}>
      <h3 id={title}>{project.name}</h3>
      <fieldset className="project-sections" aria-label="Project sections">
        {tab('general', 'General')}
        {tab('checks', 'Checks & merge')}
      </fieldset>
      {section === 'general' ? (
        <>
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
        </>
      ) : (
        <Checks project={project} agents={agents} fields={fields} edit={edit} />
      )}
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
      <div className="agent-actions">
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

interface ChecksProps {
  project: Project;
  agents: Agent[];
  fields: Fields;
  edit: (more: Partial<Fields>) => void;
}

/** The checks as the Supervisor set them (the person never types one), then what gates a merge. */
function Checks({ project, agents, fields, edit }: ChecksProps) {
  const heading = useId();
  const who = (source: string) =>
    source === 'found'
      ? 'Found in the project'
      : (agents.find((a) => a.id === source)?.name ??
        project.members.find((m) => m.agent === source)?.name ??
        'A removed agent');
  return (
    <>
      <div className="agent-field">
        <h4 id={heading}>Checks</h4>
        <p className="muted">Set by the project’s Supervisor. They run in order on the reviewed commit.</p>
        {project.checks.length ? (
          <ol className="project-checks" aria-labelledby={heading}>
            {project.checks.map((c) => (
              <li key={c.command}>
                <code>{c.command}</code>
                <small className="muted">
                  {who(c.source)} · {began(new Date(c.set).getTime() * 1000)}
                </small>
              </li>
            ))}
          </ol>
        ) : (
          <p className="muted">No checks yet</p>
        )}
      </div>
      <Choice
        className="project-gate"
        label="Required checks"
        options={required}
        value={fields.ci}
        onChange={(ci) => edit({ ci: ci as Project['ci'] })}
      />
      <Choice
        label="Merge method"
        options={methods}
        value={fields.merge}
        onChange={(merge) => edit({ merge: merge as Project['merge'] })}
      />
    </>
  );
}

interface ChoiceProps {
  className?: string;
  label: string;
  /** Each value, its label and what it means, said under the picker. */
  options: [string, string, string][];
  value: string;
  onChange: (value: string) => void;
}

function Choice({ className, label, options, value, onChange }: ChoiceProps) {
  const [id, about] = [useId(), useId()];
  return (
    <div className={cn('agent-field', className)}>
      <label htmlFor={id}>{label}</label>
      <select id={id} aria-describedby={about} value={value} onChange={(e) => onChange(e.target.value)}>
        {options.map(([option, name]) => (
          <option key={option} value={option}>
            {name}
          </option>
        ))}
      </select>
      <small id={about} className="muted">
        {options.find(([option]) => option === value)?.[2]}
      </small>
    </div>
  );
}
