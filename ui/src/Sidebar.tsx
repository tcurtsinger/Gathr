import type { Chat } from './api';
import './sidebar.css';

interface Props {
  chats: Chat[];
  selected: string | null;
  onSelect: (chat: string) => void;
  onNew: () => void;
  onSettings: (opener: HTMLElement) => void;
}

/*
  Lucide's "settings" icon (https://lucide.dev), under its ISC License:

  Copyright (c) 2026 Lucide Icons and Contributors

  Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is
  hereby granted, provided that the above copyright notice and this permission notice appear in all copies.

  THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE
  INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
  ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE,
  DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
  CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
*/
const gear = [
  'M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033',
  '2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0',
  '2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051',
  'a2.34 2.34 0 0 0 3.319-1.915',
].join(' ');

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
        <button type="button" className="navbutton" onClick={(e) => onSettings(e.currentTarget)}>
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d={gear} />
            <circle cx="12" cy="12" r="3" />
          </svg>
          Settings
        </button>
      </div>
    </nav>
  );
}
