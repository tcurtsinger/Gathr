import { MoreHorizontal, Pause, Play } from 'lucide-react';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { Run } from './pauses';

/**
 * A More menu (the prototype's ChatActions): Pause; Resume once paused; Restart once stopped. The
 * chat's header has it, and so does each agent's row in Summary, for that agent alone.
 */
export function ChatMenu({
  state,
  label = 'More',
  all = false,
  onPause,
  onResume,
}: {
  state: Run['state'];
  label?: string;
  /** Held by the whole app's pause: Resume resumes everything. */
  all?: boolean;
  onPause: () => void;
  /** Resume, which is Restart after Stop now. */
  onResume: () => void;
}) {
  const running = state === 'running';
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <TooltipIconButton tooltip={label}>
          <MoreHorizontal />
        </TooltipIconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" sideOffset={8}>
        <DropdownMenuItem onSelect={running ? onPause : onResume}>
          {running ? <Pause /> : <Play />}
          {running ? 'Pause' : state === 'stopped' ? 'Restart' : 'Resume'}
          {all && !running && ' all'}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
