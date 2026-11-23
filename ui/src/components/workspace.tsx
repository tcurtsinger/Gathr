import { PanelRight } from 'lucide-react';
import { IconButton } from './IconButton';
import './workspace.css';

/** DESIGN.md Layout: beside the chat it leaves the chat 360px, as workspace.css: the room from the sidebar to the frame's end. */
export function widest() {
  const nav = document.querySelector('.sidebar')?.getBoundingClientRect().right ?? 0;
  const end = document.querySelector('.workspace')?.getBoundingClientRect().right ?? innerWidth;
  return Math.max(360, end - nav - 360);
}

/** The sidebar's button in the top line, open or closed. */
/** `unseen`: a dot, for a change the person hasn't seen while it is shut. */
export function WorkspaceToggle({
  open,
  unseen,
  onToggle,
}: {
  open: boolean;
  unseen?: boolean;
  onToggle: () => void;
}) {
  return (
    <IconButton
      aria-label={open ? 'Close sidebar' : 'Open sidebar'}
      aria-expanded={open}
      className="head-button workspace-toggle"
      data-unseen={unseen || undefined}
      onClick={onToggle}
    >
      <PanelRight />
    </IconButton>
  );
}
