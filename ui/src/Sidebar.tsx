import { Plus } from 'lucide-react';
import { type ReactNode, useState } from 'react';
import type { Chat, ChatMark, Project } from './api';
import { ChatMenu } from './components/ChatMenu';
import { Face, type FaceOf } from './components/Face';
import { FROM_SERVICE, WORD } from './components/marks';
import { ResizeHandle } from './components/ResizeHandle';
import { StateMark } from './components/StateMark';
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
  /** Settings > Projects, at this project. */
  onProjectSettings: (project: string, opener: HTMLElement | null) => void;
  onDeleteProject: (project: Project, opener: HTMLElement | null) => void;
  /** Under New: the Tasks flyout. */
  actions?: ReactNode;
  /** Under Search: the assistant's row, lit while its chat is open, with its chat's mark. */
  assistant?: { agent: FaceOf; open: boolean; mark?: ChatMark | null; onOpen: () => void };
  onRename: (chat: Chat, opener: HTMLElement | null) => void;
  onArchive: (chat: Chat, opener: HTMLElement | null) => void;
  onDelete: (chat: Chat, opener: HTMLElement | null) => void;
  onRestore: (chat: Chat) => void;
}

export function Sidebar(props: Props) {
  const { archived, projects, selected, onSelect, onNew, actions, assistant } = props;
  // The assistant's chat is its row, in no list.
  const chats = props.chats.filter((c) => !c.assistant);
  // Projects and Chats start open, Archived folded; the person's fold of one holds while the window is open.
  const [folded, setFolded] = useState<Set<string>>(new Set());
  const [chatsOpen, setChatsOpen] = useState(true);
  const [archiveOpen, setArchiveOpen] = useState(false);
  const plain = chats.filter((c) => !c.project || !projects.some((p) => p.id === c.project));
  const row = (c: Chat) => (
    <div key={c.id} className="chatrow" data-current={c.id === selected || undefined}>
      <button
        type="button"
        className="chat-link"
        aria-current={c.id === selected ? 'page' : undefined}
        onClick={() => onSelect(c.id)}
      >
        <span>{c.title}</span>
        {c.mark && (
          <i className="chat-mark" role="img" aria-label={WORD[FROM_SERVICE[c.mark]]}>
            <StateMark state={FROM_SERVICE[c.mark]} size={9} spin={c.mark === 'working'} />
          </i>
        )}
      </button>
      <ChatMenu
        label={`More for ${c.title}`}
        className="row-icon row-more"
        archived={!!c.archived}
        onRename={(opener) => props.onRename(c, opener)}
        onArchive={(opener) => props.onArchive(c, opener)}
        onRestore={() => props.onRestore(c)}
        onDelete={(opener) => props.onDelete(c, opener)}
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
        {assistant && (
          <button
            type="button"
            className="navbutton assistant-row"
            aria-current={assistant.open ? 'page' : undefined}
            onClick={assistant.onOpen}
          >
            <Face agent={assistant.agent} size={16} />
            <span>{assistant.agent.name}</span>
            {assistant.mark && (
              <i className="chat-mark" role="img" aria-label={WORD[FROM_SERVICE[assistant.mark]]}>
                <StateMark
                  state={FROM_SERVICE[assistant.mark]}
                  size={9}
                  spin={assistant.mark === 'working'}
                />
              </i>
            )}
          </button>
        )}
      </div>
      <div className="chatlist">
        {projects.map((p) => {
          const open = !folded.has(p.id);
          const own = chats.filter((c) => c.project === p.id);
          return (
            <section key={p.id}>
              <div className="project-row">
                <h2>
                  <button
                    type="button"
                    className="list-header"
                    aria-expanded={open}
                    onClick={() => {
                      const next = new Set(folded);
                      if (open) next.add(p.id);
                      else next.delete(p.id);
                      setFolded(next);
                    }}
                  >
                    <span>{p.name}</span>
                    {/* Closed, a project shows its count as Archived does. */}
                    {!open && <span className="nav-count">{own.length}</span>}
                  </button>
                </h2>
                <button
                  type="button"
                  className="row-icon"
                  aria-label={`New chat in ${p.name}`}
                  onClick={() => onNew(p.id)}
                >
                  <Plus aria-hidden="true" />
                </button>
                <ChatMenu
                  label={`More for ${p.name}`}
                  className="row-icon row-more"
                  onProjectSettings={(opener) => props.onProjectSettings(p.id, opener)}
                  onDelete={(opener) => props.onDeleteProject(p, opener)}
                />
              </div>
              {open && own.map(row)}
            </section>
          );
        })}
        {plain.length > 0 && (
          <section>
            {/* With no projects, Chats is the only list and has no header to fold it by. */}
            {projects.length > 0 && (
              <h2>
                <button
                  type="button"
                  className="list-header"
                  aria-expanded={chatsOpen}
                  onClick={() => setChatsOpen(!chatsOpen)}
                >
                  Chats
                  {!chatsOpen && <span className="nav-count">{plain.length}</span>}
                </button>
              </h2>
            )}
            {(chatsOpen || projects.length === 0) && plain.map(row)}
          </section>
        )}
        <section>
          <h2>
            <button
              type="button"
              className="list-header"
              aria-expanded={archiveOpen}
              onClick={(e) => {
                setArchiveOpen(!archiveOpen);
                // At the foot of a long list, opening it would show nothing new without this.
                const group = e.currentTarget.closest('section');
                if (!archiveOpen) requestAnimationFrame(() => group?.scrollIntoView({ block: 'nearest' }));
              }}
            >
              Archived
              <span className="nav-count">{archived.length}</span>
            </button>
          </h2>
          {archiveOpen &&
            (archived.length ? archived.map(row) : <p className="sidebar-empty">No archived chats</p>)}
        </section>
      </div>
      <ResizeHandle label="Resize navigation" side="right" name="--sidebar-width" min={190} max={() => 400} />
    </nav>
  );
}
