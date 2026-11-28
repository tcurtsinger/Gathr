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
export function WorkspaceToggle({ open, onToggle }: { open: boolean; onToggle: () => void }) {
  return (
    <IconButton
      aria-label={open ? 'Close sidebar' : 'Open sidebar'}
      aria-expanded={open}
      className="head-button workspace-toggle"
      onClick={onToggle}
    >
      <PanelRight />
    </IconButton>
  );
}
