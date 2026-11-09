import { ChevronDown, ChevronRight, Folder, FolderOpen, Plus, Settings } from 'lucide-react';
import { type ReactNode, useState } from 'react';
import type { Chat, Project } from './api';
import { ChatMenu } from './chat/ChatMenu';
import './sidebar.css';

interface Props {
  chats: Chat[];
  /** Archived chats, last archived first (F11): in a folded group after the others. */
  archived: Chat[];
  projects: Project[];
  selected: string | null;
  onSelect: (chat: string) => void;
  /** A new chat, in the project given. */
  onNew: (project?: string) => void;
  onSettings: (opener: HTMLElement) => void;
  /** Under New: the Tasks flyout. */
  actions?: ReactNode;
  onArchive: (chat: Chat) => void;
  onDelete: (chat: Chat) => void;
  onRestore: (chat: Chat) => void;
}

export function Sidebar(props: Props) {
  const { chats, archived, projects, selected, onSelect, onNew, onSettings, actions } = props;
  // Projects start open, Archived folded; the person's fold of one holds while the window is open.
  const [folded, setFolded] = useState<Set<string>>(new Set());
  const [archiveOpen, setArchiveOpen] = useState(false);
  const plain = chats.filter((c) => !c.project || !projects.some((p) => p.id === c.project));
  const row = (c: Chat) => (
    <div key={c.id} className="chatrow" data-current={c.id === selected || undefined}>
      <button
        type="button"
        className="chat-link"
        aria-current={c.id === selected ? 'page' : undefined}
        title={c.title}
        onClick={() => onSelect(c.id)}
      >
        <span>{c.title}</span>
      </button>
      <ChatMenu
        label={`More for ${c.title}`}
        className="row-more"
        archived={!!c.archived}
        onArchive={() => props.onArchive(c)}
        onRestore={() => props.onRestore(c)}
        onDelete={() => props.onDelete(c)}
      />
    </div>
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
        {actions}
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
        <section className="archive-group">
          <button
            type="button"
            className="navbutton"
            aria-expanded={archiveOpen}
            onClick={() => setArchiveOpen(!archiveOpen)}
          >
            {archiveOpen ? <ChevronDown aria-hidden="true" /> : <ChevronRight aria-hidden="true" />}
            Archived
            {archived.length > 0 && <span className="nav-count">{archived.length}</span>}
          </button>
          {archiveOpen &&
            (archived.length ? archived.map(row) : <p className="sidebar-empty">No archived chats</p>)}
        </section>
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
