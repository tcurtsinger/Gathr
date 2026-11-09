import { Folder, FolderOpen, Plus, Settings } from 'lucide-react';
import { useState } from 'react';
import type { Chat, Project } from './api';
import './sidebar.css';

interface Props {
  chats: Chat[];
  projects: Project[];
  selected: string | null;
  onSelect: (chat: string) => void;
  /** A new chat, in the project given. */
  onNew: (project?: string) => void;
  onSettings: (opener: HTMLElement) => void;
}

export function Sidebar({ chats, projects, selected, onSelect, onNew, onSettings }: Props) {
  // Projects start open; the person's fold of one holds while the window is open.
  const [folded, setFolded] = useState<Set<string>>(new Set());
  const plain = chats.filter((c) => !c.project || !projects.some((p) => p.id === c.project));
  const row = (c: Chat) => (
    <button
      type="button"
      key={c.id}
      className="chatrow"
      aria-current={c.id === selected ? 'page' : undefined}
      title={c.title}
      onClick={() => onSelect(c.id)}
    >
      <span>{c.title}</span>
    </button>
  );
  return (
    <nav className="sidebar" aria-label="Chats">
      <div className="brand">
        <span className="logo" aria-hidden="true">
          <i />
          <i />
        </span>
        <strong>Gathr</strong>
      </div>
      <div className="sidebar-actions">
        <button type="button" className="navbutton" onClick={() => onNew()}>
          <Plus aria-hidden="true" />
          New
        </button>
      </div>
      <div className="chatlist">
        {projects.length > 0 && (
          <section>
            <h2>Projects</h2>
            {projects.map((p) => {
              const open = !folded.has(p.id);
              return (
                <div key={p.id}>
                  <div className="project-row">
                    <button
                      type="button"
                      className="navbutton"
                      aria-expanded={open}
                      title={p.folder}
                      onClick={() => {
                        const next = new Set(folded);
                        if (open) next.add(p.id);
                        else next.delete(p.id);
                        setFolded(next);
                      }}
                    >
                      {open ? <FolderOpen aria-hidden="true" /> : <Folder aria-hidden="true" />}
                      <span>{p.name}</span>
                    </button>
                    <button
                      type="button"
                      className="navbutton project-new"
                      aria-label={`New chat in ${p.name}`}
                      title={`New chat in ${p.name}`}
                      onClick={() => onNew(p.id)}
                    >
                      <Plus aria-hidden="true" />
                    </button>
                  </div>
                  {open && (
                    <div className="project-chats">{chats.filter((c) => c.project === p.id).map(row)}</div>
                  )}
                </div>
              );
            })}
          </section>
        )}
        {plain.length > 0 && (
          <section>
            {projects.length > 0 && <h2>Chats</h2>}
            {plain.map(row)}
          </section>
        )}
      </div>
      <div className="sidebar-foot">
        <button type="button" className="navbutton" onClick={(e) => onSettings(e.currentTarget)}>
          <Settings aria-hidden="true" />
          Settings
        </button>
      </div>
    </nav>
  );
}
