import { MoreHorizontal, Pause, Play } from 'lucide-react';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { Run, RunAction } from '../api';

const label: Record<RunAction, string> = { pause: 'Pause', resume: 'Resume', restart: 'Restart' };

/** The chat header's More menu (the prototype's ChatActions): Pause, Resume once paused, Restart once stopped. */
export function ChatMenu({ run, onRun }: { run: Run; onRun: (action: RunAction) => void }) {
  const action: RunAction =
    run.state === 'running' ? 'pause' : run.state === 'stopped' ? 'restart' : 'resume';
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <TooltipIconButton tooltip="More">
          <MoreHorizontal />
        </TooltipIconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" sideOffset={8}>
        <DropdownMenuItem onSelect={() => onRun(action)}>
          {action === 'pause' ? <Pause /> : <Play />}
          {label[action]}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
