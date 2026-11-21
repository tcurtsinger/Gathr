import { File as FileIcon, Files, House, X } from 'lucide-react';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { IconButton } from '../components/IconButton';
import { ResizeHandle } from '../components/ResizeHandle';
import { widest } from '../components/workspace';
import { Board } from './Board';
import { FileTab, type Opened, Tree } from './Files';
import './panel.css';

/** A chat's open tabs, and the one shown: Home, Files, or a file's path. */
interface Tabs {
  files: boolean;
  open: Opened[];
  at: string;
}
const HOME = 'home';
const FILES = 'files';
// Each chat's tabs, while the window is open.
const kept = new Map<string, Tabs>();

/** The panel beside the chat: Home first, always there, then Files and each open file as tabs. */
export function RoomSidebar({ chat }: { chat: string }) {
  return (
    <aside className="workspace" aria-label="Chat sidebar">
      <ResizeHandle label="Resize chat sidebar" side="left" name="--workspace-width" min={360} max={widest} />
      <Panel key={chat} chat={chat} />
    </aside>
  );
}

function Panel({ chat }: { chat: string }) {
  const [tabs, setTabs] = useState<Tabs>(() => kept.get(chat) ?? { files: false, open: [], at: HOME });
  const change = (next: Tabs) => {
    kept.set(chat, next);
    setTabs(next);
  };
  const order = [HOME, ...(tabs.files ? [FILES] : []), ...tabs.open.map((f) => f.path)];
  const go = (at: string) => change({ ...tabs, at });
  const close = (key: string) => {
    const i = order.indexOf(key);
    const at = key === tabs.at ? (order[i + 1] ?? order[i - 1]) : tabs.at;
    change({ files: tabs.files && key !== FILES, open: tabs.open.filter((f) => f.path !== key), at });
  };
  const open = (file: Opened) => {
    const known = tabs.open.some((f) => f.path === file.path);
    change({ ...tabs, open: known ? tabs.open : [...tabs.open, file], at: file.path });
  };
  const id = (key: string) => order.indexOf(key);
  const tab = (key: string, icon: ReactNode, name: string, closable = true) => (
    <Tab
      key={key}
      id={id(key)}
      selected={tabs.at === key}
      icon={icon}
      name={name}
      onSelect={() => go(key)}
      onClose={closable ? () => close(key) : undefined}
    />
  );
  const view = (key: string, body: ReactNode) => (
    <section
      key={key}
      id={`room-view-${id(key)}`}
      role="tabpanel"
      aria-labelledby={`room-tab-${id(key)}`}
      className="workspace-view"
      hidden={tabs.at !== key}
    >
      {body}
    </section>
  );
  return (
    <>
      <div className="workspace-tabs">
        <div className="workspace-tablist" role="tablist" aria-label="Sidebar tabs">
          {tab(HOME, <House aria-hidden="true" />, 'Home', false)}
          {tabs.files && tab(FILES, <Files aria-hidden="true" />, 'Files')}
          <div className="workspace-files">
            {tabs.open.map((f) => tab(f.path, <FileIcon aria-hidden="true" />, f.crumbs.at(-1) ?? f.path))}
          </div>
        </div>
      </div>
      {view(
        HOME,
        <div className="home">
          <h2>Open</h2>
          <button
            type="button"
            className="home-tile"
            onClick={() => change({ ...tabs, files: true, at: FILES })}
          >
            <Files aria-hidden="true" />
            Files
          </button>
          <h2>Board</h2>
          <Board chat={chat} />
        </div>,
      )}
      {tabs.files && view(FILES, <Tree chat={chat} onOpen={open} />)}
      {tabs.open.map((f) => view(f.path, <FileTab chat={chat} file={f} />))}
    </>
  );
}

interface TabProps {
  id: number;
  selected: boolean;
  icon: ReactNode;
  name: string;
  onSelect: () => void;
  onClose?: () => void;
}

/** One tab: the open one scrolls into sight. */
function Tab({ id, selected, icon, name, onSelect, onClose }: TabProps) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (selected) ref.current?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [selected]);
  return (
    <div ref={ref} className="workspace-tab" data-selected={selected}>
      <button
        type="button"
        role="tab"
        id={`room-tab-${id}`}
        aria-selected={selected}
        aria-controls={`room-view-${id}`}
        onClick={onSelect}
      >
        {icon}
        <span>{name}</span>
      </button>
      {onClose && (
        <IconButton aria-label={`Close ${name}`} className="workspace-close" onClick={onClose}>
          <X />
        </IconButton>
      )}
    </div>
  );
}
