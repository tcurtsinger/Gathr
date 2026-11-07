import type { Chat } from './api';
import './sidebar.css';

interface Props {
  chats: Chat[];
  selected: string | null;
  onSelect: (chat: string) => void;
  onNew: () => void;
  onSettings: (opener: HTMLElement) => void;
}

export function Sidebar({ chats, selected, onSelect, onNew, onSettings }: Props) {
  return (
    <nav className="sidebar" aria-label="Chats">
      <div className="brand">
        <span className="logo" aria-hidden="true">
          <i />
          <i />
        </span>
        <strong>Gathr</strong>
      </div>
      <button type="button" className="navbutton" onClick={onNew}>
        <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
          <path d="M8 3v10M3 8h10" fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
        New
      </button>
      <div className="chatlist">
        {chats.map((c) => (
          <button
            type="button"
            key={c.id}
            className="chatrow"
            aria-current={c.id === selected ? 'page' : undefined}
            onClick={() => onSelect(c.id)}
          >
            <span>{c.title}</span>
          </button>
        ))}
      </div>
      <div className="sidebar-foot">
        <button type="button" onClick={(e) => onSettings(e.currentTarget)}>
          Settings
        </button>
      </div>
    </nav>
  );
}
