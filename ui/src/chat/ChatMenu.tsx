import { Archive, ArchiveRestore, MoreHorizontal, Pause, Play, Settings, Trash2 } from 'lucide-react';
import { Fragment, type ReactElement, useRef } from 'react';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { Run } from './pauses';

interface Props {
  label?: string;
  /** The pause item: Pause; Resume once paused; Restart once stopped. None for a sidebar row. */
  pause?: {
    state: Run['state'];
    /** Held by the whole app's pause: Resume resumes everything. */
    all?: boolean;
    onPause: () => void;
    /** Resume, which is Restart after Stop now. */
    onResume: () => void;
  };
  /** An archived chat offers Restore where a listed one offers Archive (F11). */
  archived?: boolean;
  /** Each gets the menu's button, for a confirm to give focus back to. */
  onArchive?: (opener: HTMLElement | null) => void;
  onRestore?: () => void;
  onDelete?: (opener: HTMLElement | null) => void;
  /** A project's row: its settings (0036). */
  onProjectSettings?: (opener: HTMLElement | null) => void;
  className?: string;
  /** Where the menu lines up with its button: 'start' beside the chat title, 'end' at a row's end. */
  align?: 'start' | 'end';
  /** Where its tooltip shows: left in a list, so it never covers the next row's button. */
  side?: 'bottom' | 'left';
}

/**
 * A More menu (the prototype's ChatActions): the pause item, then Archive or Restore, then Delete,
 * each after a separator. The chat's header has them all, a sidebar row Archive and Delete, each
 * agent's row in Summary only the pause item, for that agent alone, and a project's row Project settings.
 */
export function ChatMenu({
  label = 'More',
  pause,
  archived,
  onArchive,
  onRestore,
  onDelete,
  onProjectSettings,
  className,
  align = 'end',
  side,
}: Props) {
  const running = pause?.state === 'running';
  const trigger = useRef<HTMLButtonElement>(null);
  const items: [string, ReactElement][] = [];
  if (pause && !archived)
    items.push([
      'pause',
      <DropdownMenuItem key="pause" onSelect={running ? pause.onPause : pause.onResume}>
        {running ? <Pause /> : <Play />}
        {running ? 'Pause' : pause.state === 'stopped' ? 'Restart' : 'Resume'}
        {pause.all && !running && ' all'}
      </DropdownMenuItem>,
    ]);
  if (archived && onRestore)
    items.push([
      'restore',
      <DropdownMenuItem key="restore" onSelect={onRestore}>
        <ArchiveRestore />
        Restore
      </DropdownMenuItem>,
    ]);
  if (!archived && onArchive)
    items.push([
      'archive',
      <DropdownMenuItem key="archive" onSelect={() => onArchive(trigger.current)}>
        <Archive />
        Archive
      </DropdownMenuItem>,
    ]);
  if (onProjectSettings)
    items.push([
      'project',
      <DropdownMenuItem key="project" onSelect={() => onProjectSettings(trigger.current)}>
        <Settings />
        Project settings
      </DropdownMenuItem>,
    ]);
  if (onDelete)
    items.push([
      'delete',
      <DropdownMenuItem key="delete" variant="destructive" onSelect={() => onDelete(trigger.current)}>
        <Trash2 />
        Delete
      </DropdownMenuItem>,
    ]);
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <TooltipIconButton ref={trigger} tooltip={label} side={side} className={className}>
          <MoreHorizontal />
        </TooltipIconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent align={align} sideOffset={8}>
        {items.map(([key, item], i) => (
          <Fragment key={key}>
            {i > 0 && <DropdownMenuSeparator />}
            {item}
          </Fragment>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
