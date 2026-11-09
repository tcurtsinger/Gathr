import { Archive, ArchiveRestore, MoreHorizontal, Pause, Play, Trash2 } from 'lucide-react';
import { Fragment, type ReactElement } from 'react';
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
  onArchive?: () => void;
  onRestore?: () => void;
  onDelete?: () => void;
  className?: string;
}

/**
 * A More menu (the prototype's ChatActions): the pause item, then Archive or Restore, then Delete,
 * each after a separator. The chat's header has them all, a sidebar row Archive and Delete, and each
 * agent's row in Summary only the pause item, for that agent alone.
 */
export function ChatMenu({
  label = 'More',
  pause,
  archived,
  onArchive,
  onRestore,
  onDelete,
  className,
}: Props) {
  const running = pause?.state === 'running';
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
      <DropdownMenuItem key="archive" onSelect={onArchive}>
        <Archive />
        Archive
      </DropdownMenuItem>,
    ]);
  if (onDelete)
    items.push([
      'delete',
      <DropdownMenuItem key="delete" variant="destructive" onSelect={onDelete}>
        <Trash2 />
        Delete
      </DropdownMenuItem>,
    ]);
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <TooltipIconButton tooltip={label} className={className}>
          <MoreHorizontal />
        </TooltipIconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" sideOffset={8}>
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
