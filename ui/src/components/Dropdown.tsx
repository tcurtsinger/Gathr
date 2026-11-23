import { CheckIcon, ChevronDown } from 'lucide-react';
import { Select } from 'radix-ui';
import { type ComponentProps, type ReactNode, useCallback, useState } from 'react';
import { cn } from '@/lib/utils';
import './dropdown.css';

export interface Choice {
  value: string;
  label: string;
  /** Before the label, in the list and closed: an agent's face, an engine's logo. */
  icon?: ReactNode;
}

type Props = Omit<ComponentProps<'button'>, 'value' | 'onChange' | 'children'> & {
  value: string;
  choices: Choice[];
  onChange: (value: string) => void;
  /** Shown while the value is none of the choices. */
  placeholder?: string;
  /** What the closed dropdown reads instead of the chosen choice's label. */
  shown?: ReactNode;
  /** Above the field rather than below: the pickers over the message box. With no room there, it turns by itself. */
  side?: 'top';
};

// Radix keeps '' for "nothing chosen", so a choice whose value is '' goes by this inside.
const EMPTY = '\u0000';
const inner = (value: string) => (value === '' ? EMPTY : value);

// The registry's DropdownMenuContent and its checkbox item, so the list looks as the More menu does.
const list = cn(
  'z-50 max-h-(--radix-select-content-available-height) min-w-(--radix-select-trigger-width)',
  'origin-(--radix-select-content-transform-origin) overflow-x-hidden overflow-y-auto rounded-md border bg-popover',
  'text-popover-foreground shadow-md data-[side=bottom]:slide-in-from-top-2 data-[side=top]:slide-in-from-bottom-2',
  'data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95',
);
const item = cn(
  'relative flex cursor-default items-center rounded-sm py-1.5 pr-2 pl-8 text-sm outline-hidden select-none',
  'focus:bg-accent focus:text-accent-foreground',
);

/** Gathr's one dropdown, drawn by the app: closed, a field with the chosen value; open, a menu of the choices. */
export function Dropdown({
  value,
  choices,
  onChange,
  placeholder,
  shown,
  side,
  disabled,
  className,
  ...trigger
}: Props) {
  // A modal <dialog> is in the top layer: a list put in the body would open behind it.
  const [host, setHost] = useState<HTMLElement | null>(null);
  const place = useCallback(
    (button: HTMLButtonElement | null) => setHost(button?.closest('dialog') ?? null),
    [],
  );
  const chosen = choices.some((c) => c.value === value) ? inner(value) : '';

  return (
    <Select.Root value={chosen} onValueChange={(v) => onChange(v === EMPTY ? '' : v)} disabled={disabled}>
      <Select.Trigger ref={place} className={cn('dropdown', className)} {...trigger}>
        <Select.Value placeholder={shown ?? placeholder}>{shown}</Select.Value>
        <Select.Icon asChild>
          <ChevronDown aria-hidden="true" />
        </Select.Icon>
      </Select.Trigger>
      <Select.Portal container={host ?? undefined}>
        <Select.Content
          position="popper"
          side={side}
          sideOffset={4}
          className={list}
          // Enter picks on keydown; left to go on, it presses whatever has focus next, such as the
          // Close of the New project dialog that picking New project opens.
          onKeyDown={(e) => e.key === 'Enter' && e.preventDefault()}
        >
          <Select.Viewport className="p-1">
            {choices.map((c) => (
              <Select.Item key={c.value} value={inner(c.value)} className={item}>
                <span className="pointer-events-none absolute left-2 flex size-4 items-center justify-center">
                  <Select.ItemIndicator>
                    <CheckIcon className="size-4" />
                  </Select.ItemIndicator>
                </span>
                <Select.ItemText>
                  {c.icon ? (
                    <span className="dropdown-choice">
                      {c.icon}
                      {c.label}
                    </span>
                  ) : (
                    c.label
                  )}
                </Select.ItemText>
              </Select.Item>
            ))}
          </Select.Viewport>
        </Select.Content>
      </Select.Portal>
    </Select.Root>
  );
}
