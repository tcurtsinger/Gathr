import { Folder } from 'lucide-react';
import type { Project } from '../api';
import { Dropdown } from './Dropdown';
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
    <div className="project-pick">
      <Folder aria-hidden="true" />
      <Dropdown
        aria-label="Project"
        value={project ?? ''}
        onChange={(v) => (v === 'new' ? onNewProject() : onProject(v))}
        choices={[
          { value: '', label: 'No project' },
          ...projects.map((p) => ({ value: p.id, label: p.name })),
          { value: 'new', label: 'New project' },
        ]}
      />
    </div>
  );
}
