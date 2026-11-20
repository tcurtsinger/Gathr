import { Folder } from 'lucide-react';
import type { Project } from '../api';
import './project-pick.css';

/** A new chat's project, picked above the message box before its first message; '' for none. */
export function ProjectPick({
  project,
  projects,
  onProject,
  onNewProject,
}: {
  project?: string | null;
  projects: Project[];
  onProject: (project: string) => void;
  onNewProject: () => void;
}) {
  return (
    <label className="project-pick">
      <Folder aria-hidden="true" />
      <select
        aria-label="Project"
        value={project ?? ''}
        onChange={(e) => (e.target.value === 'new' ? onNewProject() : onProject(e.target.value))}
      >
        <option value="">No project</option>
        {projects.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
        <option value="new">New project</option>
      </select>
    </label>
  );
}
