import { Plus, Settings } from 'lucide-react';
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
      <div className="sidebar-actions">
        <button type="button" className="navbutton" onClick={onNew}>
          <Plus aria-hidden="true" />
          New
        </button>
      </div>
      <div className="chatlist">
        {chats.map((c) => (
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
        ))}
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
