import { PanelRight } from 'lucide-react';
import { IconButton } from './IconButton';
import './workspace.css';

/** DESIGN.md Layout: beside the chat it leaves the chat 360px, as workspace.css: the room from the sidebar to the frame's end. */
export function widest() {
  const nav = document.querySelector('.sidebar')?.getBoundingClientRect().right ?? 0;
  const end = document.querySelector('.workspace')?.getBoundingClientRect().right ?? innerWidth;
  return Math.max(360, end - nav - 360);
}

/** The panel's button in the top line, open or closed; closed, its dot says a question waits on the person. */
export function WorkspaceToggle(props: { open: boolean; waiting: boolean; onToggle: () => void }) {
  const { open, waiting, onToggle } = props;
  return (
    <IconButton
      aria-label={open ? 'Close sidebar' : 'Open sidebar'}
      aria-expanded={open}
      className="head-button workspace-toggle"
      onClick={onToggle}
    >
      <PanelRight />
      {waiting && !open && <i className="attention" />}
    </IconButton>
  );
}
