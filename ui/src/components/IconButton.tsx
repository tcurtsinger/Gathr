import type { ComponentPropsWithRef } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

type Props = ComponentPropsWithRef<typeof Button> & { 'aria-label': string };

/**
 * The one icon-only button: the registry's Button drawn as its TooltipIconButton draws it, named for
 * screen readers by its label and with no tooltip (DESIGN.md: no hover help on a control).
 */
export function IconButton({ className, ...rest }: Props) {
  return (
    <Button
      variant="ghost"
      size="icon"
      {...rest}
      className={cn('aui-button-icon size-6 p-1 active:scale-90', className)}
    />
  );
}
