import { X } from 'lucide-react';
import { Popover } from 'radix-ui';
import type { ReactNode } from 'react';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import './flyout.css';

interface Props {
  title: string;
  /** The sidebar button's content: its icon, name and count or key. */
  trigger: ReactNode;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** What takes focus as it opens; its first button otherwise. */
  focus?: (panel: HTMLElement) => HTMLElement | null;
  /** Focus goes back to its trigger as it closes unless this prevents it. */
  onCloseAutoFocus?: (e: Event) => void;
  children: ReactNode;
}

/**
 * The flyout beside navigation that Search and Tasks share (DESIGN.md "Navigation and workspace"):
 * the prototype's Radix popover, so its own tooltips draw above it. Escape, an outside click, Close
 * or its trigger closes it.
 */
export function NavFlyout({ title, trigger, open, onOpenChange, focus, onCloseAutoFocus, children }: Props) {
  return (
    <Popover.Root open={open} onOpenChange={onOpenChange}>
      <Popover.Trigger asChild>
        <button type="button" className="navbutton">
          {trigger}
        </button>
      </Popover.Trigger>
      <Popover.Anchor asChild>
        <span className="flyout-anchor" aria-hidden="true" />
      </Popover.Anchor>
      <Popover.Portal>
        <Popover.Content
          className="flyout"
          side="right"
          align="start"
          sideOffset={8}
          collisionPadding={8}
          role="dialog"
          aria-label={title}
          onOpenAutoFocus={(e) => {
            e.preventDefault();
            const panel = e.currentTarget as HTMLElement;
            (focus?.(panel) ?? panel.querySelector('button'))?.focus();
          }}
          onCloseAutoFocus={onCloseAutoFocus}
        >
          <header>
            <h2>{title}</h2>
            <TooltipIconButton tooltip={`Close ${title}`} onClick={() => onOpenChange(false)}>
              <X />
            </TooltipIconButton>
          </header>
          {children}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
