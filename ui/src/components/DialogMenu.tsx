import { DropdownMenu } from 'radix-ui';
import type { ComponentProps } from 'react';
import { cn } from '@/lib/utils';

// The registry's DropdownMenuContent, as it draws it.
const content = cn(
  'z-50 max-h-(--radix-dropdown-menu-content-available-height) min-w-[8rem]',
  'origin-(--radix-dropdown-menu-content-transform-origin) overflow-x-hidden overflow-y-auto rounded-md border',
  'bg-popover p-1 text-popover-foreground shadow-md data-[side=bottom]:slide-in-from-top-2',
  'data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2',
  'data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95',
  'data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95',
);

type Props = ComponentProps<typeof DropdownMenu.Content> & { host: HTMLElement | null };

/**
 * The registry's More menu list, put inside the modal dialog it opens from (Settings): the registry puts it in the
 * body, which a modal dialog makes inert and draws over. Its items are the registry's own.
 */
export function DialogMenuContent({ host, className, sideOffset = 4, ...props }: Props) {
  return (
    <DropdownMenu.Portal container={host ?? undefined}>
      <DropdownMenu.Content
        data-slot="dropdown-menu-content"
        sideOffset={sideOffset}
        className={cn(content, className)}
        {...props}
      />
    </DropdownMenu.Portal>
  );
}
