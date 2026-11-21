import { CircleCheck, CircleOff, KeyRound, MoreHorizontal, Trash2 } from 'lucide-react';
import { useRef } from 'react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { WebTool } from '../api';
import { IconButton } from '../components/IconButton';

interface Props {
  tool: WebTool;
  /** Each gets the menu's button, for focus to go back to. */
  onChangeKey: (opener: HTMLElement | null) => void;
  onEnable: (on: boolean) => void;
  onRemove: (opener: HTMLElement | null) => void;
}

/** A tool row's More: Change key, Disable or Enable, then Remove. */
export function ToolMenu({ tool, onChangeKey, onEnable, onRemove }: Props) {
  const trigger = useRef<HTMLButtonElement>(null);
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <IconButton ref={trigger} aria-label={`More for ${tool.name}`} className="plugin-more">
          <MoreHorizontal />
        </IconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" sideOffset={8}>
        <DropdownMenuItem onSelect={() => onChangeKey(trigger.current)}>
          <KeyRound />
          Change key
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onEnable(!tool.enabled)}>
          {tool.enabled ? <CircleOff /> : <CircleCheck />}
          {tool.enabled ? 'Disable' : 'Enable'}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem variant="destructive" onSelect={() => onRemove(trigger.current)}>
          <Trash2 />
          Remove
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
