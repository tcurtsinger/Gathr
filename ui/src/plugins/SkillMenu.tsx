import { CircleCheck, CircleOff, FolderOpen, MoreHorizontal, Trash2 } from 'lucide-react';
import { useRef } from 'react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { Skill } from '../api';
import { IconButton } from '../components/IconButton';
import { desktop } from '../desktop';

interface Props {
  skill: Skill;
  onOpen: () => void;
  onEnable: (on: boolean) => void;
  /** Gets the menu's button, for the confirm to give focus back to. */
  onRemove: (opener: HTMLElement | null) => void;
}

/** A skill row's More: Open folder (in the desktop window), Disable or Enable, then Remove. */
export function SkillMenu({ skill, onOpen, onEnable, onRemove }: Props) {
  const trigger = useRef<HTMLButtonElement>(null);
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <IconButton ref={trigger} aria-label={`More for ${skill.name}`} className="plugin-more">
          <MoreHorizontal />
        </IconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" sideOffset={8}>
        {desktop() && (
          <DropdownMenuItem onSelect={onOpen}>
            <FolderOpen />
            Open folder
          </DropdownMenuItem>
        )}
        <DropdownMenuItem onSelect={() => onEnable(!skill.enabled)}>
          {skill.enabled ? <CircleOff /> : <CircleCheck />}
          {skill.enabled ? 'Disable' : 'Enable'}
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
