import { FileText, ListTodo, PanelRight, X } from 'lucide-react';
import type { KeyboardEvent, ReactNode } from 'react';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import { Initial } from '../chat/Initial';
import { ResizeHandle } from '../components/ResizeHandle';
import './workspace.css';

/** A file a question came with: the service's copy, made when it was asked. */
export interface FileTab {
  task: number;
  index: number;
  name: string;
}

export const fileId = (f: FileTab) => `${f.task}/${f.index}`;

/** DESIGN.md Layout: beside the chat it leaves the chat 360px; over it (1200px and below), as workspace.css. */
function widest() {
  const nav = document.querySelector('.sidebar')?.getBoundingClientRect().width ?? 0;
  return innerWidth > 1200 ? Math.max(360, innerWidth - nav - 360) : innerWidth - 48;
}

interface Props {
  /** A question waits on the person in this chat. */
  waiting: boolean;
  files: FileTab[];
  /** The agent opened from Summary or the timeline: its tab's name and its panel. One at a time. */
  agent?: { name: string; panel: ReactNode };
  /** 'tasks', 'agent', or a file tab's id. */
  selected: string;
  onSelect: (id: string) => void;
  onCloseFile: (id: string) => void;
  onCloseAgent: () => void;
  onClose: () => void;
  tasks: ReactNode;
}

/**
 * The chat's workspace beside the conversation (DESIGN.md "Navigation and workspace"): Tasks, the agent
 * opened from Summary, then a tab for each file opened from a question. The prototype's other tabs come
 * with their features.
 */
export function Workspace(props: Props) {
  const { waiting, files, agent, onSelect, onCloseFile, onCloseAgent, onClose, tasks } = props;
  // An agent that left the chat takes its tab with it.
  const selected = props.selected === 'agent' && !agent ? 'tasks' : props.selected;
  const ids = ['tasks', ...(agent ? ['agent'] : []), ...files.map(fileId)];
  const close = (id: string) => (id === 'agent' ? onCloseAgent() : onCloseFile(id));
  const keys = (e: KeyboardEvent) => {
    const at = ids.indexOf(selected);
    const to = { ArrowRight: at + 1, ArrowLeft: at - 1, Home: 0, End: ids.length - 1 }[e.key];
    if (e.key === 'Delete' && selected !== 'tasks') close(selected);
    if (to === undefined) return;
    e.preventDefault();
    const id = ids[(to + ids.length) % ids.length];
    onSelect(id);
    document.getElementById(`workspace-tab-${id}`)?.focus();
  };
  const tab = (id: string, label: string, icon: ReactNode, extra?: ReactNode) => (
    <div className="workspace-tab" key={id} data-selected={id === selected}>
      <button
        type="button"
        role="tab"
        id={`workspace-tab-${id}`}
        aria-selected={id === selected}
        aria-controls={`workspace-view-${id}`}
        tabIndex={id === selected ? 0 : -1}
        title={label}
        onClick={() => onSelect(id)}
      >
        {icon}
        <span>{label}</span>
        {extra}
      </button>
      {id !== 'tasks' && (
        <button
          type="button"
          className="workspace-tab-close"
          aria-label={`Close ${label}`}
          onClick={() => close(id)}
        >
          <X aria-hidden="true" />
        </button>
      )}
    </div>
  );
  return (
    <aside className="workspace" aria-label="Chat workspace">
      <ResizeHandle label="Resize chat sidebar" side="left" name="--workspace-width" min={360} max={widest} />
      <div className="workspace-tabs">
        <div
          className="workspace-tablist"
          role="tablist"
          aria-label="Workspace tabs"
          tabIndex={-1}
          onKeyDown={keys}
        >
          {tab('tasks', 'Tasks', <ListTodo aria-hidden="true" />, waiting && <i className="attention" />)}
          {agent && tab('agent', agent.name, <Initial name={agent.name} size="s" />)}
          {files.map((f) => tab(fileId(f), f.name, <FileText aria-hidden="true" />))}
        </div>
        <TooltipIconButton
          tooltip="Close sidebar"
          aria-expanded={true}
          className="head-button workspace-toggle"
          onClick={onClose}
        >
          <PanelRight />
        </TooltipIconButton>
      </div>
      <section
        id="workspace-view-tasks"
        role="tabpanel"
        aria-labelledby="workspace-tab-tasks"
        className="workspace-view"
        hidden={selected !== 'tasks'}
      >
        {tasks}
      </section>
      {agent && (
        <section
          id="workspace-view-agent"
          role="tabpanel"
          aria-labelledby="workspace-tab-agent"
          className="workspace-view"
          hidden={selected !== 'agent'}
        >
          {agent.panel}
        </section>
      )}
      {files.map((f) => (
        <section
          key={fileId(f)}
          id={`workspace-view-${fileId(f)}`}
          role="tabpanel"
          aria-labelledby={`workspace-tab-${fileId(f)}`}
          className="workspace-view"
          hidden={selected !== fileId(f)}
        >
          {/* Served with CSP sandbox: a page shows but runs nothing. */}
          <iframe title={f.name} src={`/api/tasks/${f.task}/files/${f.index}`} sandbox="" />
        </section>
      ))}
    </aside>
  );
}

/** The chat header's opener; its dot says a question waits on the person in this chat. */
export function WorkspaceOpener({ waiting, onOpen }: { waiting: boolean; onOpen: () => void }) {
  return (
    <TooltipIconButton
      tooltip="Open sidebar"
      aria-expanded={false}
      className="head-button workspace-toggle"
      onClick={onOpen}
    >
      <PanelRight />
      {waiting && <i className="attention" />}
    </TooltipIconButton>
  );
}
