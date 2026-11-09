import { ListTodo, PanelRight } from 'lucide-react';
import type { ReactNode } from 'react';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import './workspace.css';

/**
 * The chat's workspace beside the conversation (DESIGN.md "Navigation and workspace"). Tasks is its
 * only tab so far; the prototype's other tabs come with their features.
 */
export function Workspace({
  waiting,
  onClose,
  children,
}: {
  waiting: boolean;
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <aside className="workspace" aria-label="Chat workspace">
      <div className="workspace-tabs" role="tablist" aria-label="Workspace tabs">
        <button
          type="button"
          role="tab"
          aria-selected="true"
          aria-controls="workspace-tasks"
          id="workspace-tab-tasks"
        >
          <ListTodo aria-hidden="true" />
          <span>Tasks</span>
          {waiting && <i className="attention" />}
        </button>
        <TooltipIconButton
          tooltip="Close sidebar"
          aria-expanded={true}
          className="workspace-toggle"
          onClick={onClose}
        >
          <PanelRight />
        </TooltipIconButton>
      </div>
      <section
        id="workspace-tasks"
        role="tabpanel"
        aria-labelledby="workspace-tab-tasks"
        className="workspace-view"
      >
        {children}
      </section>
    </aside>
  );
}

/** The chat header's opener; its dot says a question waits on the person in this chat. */
export function WorkspaceOpener({ waiting, onOpen }: { waiting: boolean; onOpen: () => void }) {
  return (
    <TooltipIconButton
      tooltip="Open sidebar"
      aria-expanded={false}
      className="workspace-toggle"
      onClick={onOpen}
    >
      <PanelRight />
      {waiting && <i className="attention" />}
    </TooltipIconButton>
  );
}
