import { type KeyboardEvent, useEffect, useId, useRef, useState } from 'react';
import {
  ComposerMenu,
  ComposerMenuItem,
  ComposerModelItem,
  ComposerModelTrigger,
} from '@/components/assistant-ui/elements/composer';
/** An agent the message can go to; `meta` is its role in the chat. */
export interface Recipient {
  id: string;
  name: string;
  meta?: string;
}

interface Props {
  agents: Recipient[];
  selected: string;
  onSelect: (agent: string) => void;
  /** Opens Settings > Agents at the selected agent. */
  onEdit: () => void;
}

const moves: Record<string, (at: number, n: number) => number> = {
  ArrowDown: (at, n) => (at + 1) % n,
  ArrowUp: (at, n) => (at - 1 + n) % n,
  Home: () => 0,
  End: (_, n) => n - 1,
};

/**
 * Who the next message goes to: the same as @naming it (0028). It never changes the agent's model or
 * effort (0026).
 */
export function AgentPicker({ agents, selected, onSelect, onEdit }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useId();
  const name = agents.find((a) => a.id === selected)?.name ?? '';

  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', close);
    return () => document.removeEventListener('pointerdown', close);
  }, [open]);

  const items = () => [...(root.current?.querySelectorAll<HTMLButtonElement>('[role="option"]') ?? [])];
  const dismiss = () => {
    setOpen(false);
    trigger.current?.focus();
  };
  const openAt = (first: boolean) => {
    setOpen(true);
    requestAnimationFrame(() => {
      const options = items();
      (first ? options.find((o) => o.ariaSelected === 'true') : options.at(-1))?.focus();
    });
  };
  const keyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && open) {
      e.preventDefault();
      e.stopPropagation();
      dismiss();
      return;
    }
    const move = moves[e.key];
    if (!move) return;
    e.preventDefault();
    if (!open) return openAt(e.key !== 'ArrowUp');
    const options = items();
    options[move(options.indexOf(document.activeElement as HTMLButtonElement), options.length)]?.focus();
  };
  const choose = (pick: () => void) => {
    pick();
    dismiss();
  };

  return (
    <div className="relative" ref={root}>
      <ComposerModelTrigger
        ref={trigger}
        model={name}
        open={open}
        role="combobox"
        aria-label="Agent"
        aria-controls={menu}
        aria-haspopup="listbox"
        onClick={() => setOpen(!open)}
        onKeyDown={keyDown}
      />
      <ComposerMenu
        id={menu}
        open={open}
        align="end"
        role="listbox"
        aria-label="Agents"
        inert={!open}
        onKeyDown={keyDown}
      >
        {agents.map((a) => (
          <ComposerModelItem
            key={a.id}
            role="option"
            aria-selected={a.id === selected}
            entry={{ name: a.name, meta: a.meta ?? '' }}
            selected={a.id === selected}
            onClick={() => choose(() => onSelect(a.id))}
          />
        ))}
        <ComposerMenuItem role="option" aria-selected={false} onClick={() => choose(onEdit)}>
          Edit agent
        </ComposerMenuItem>
      </ComposerMenu>
    </div>
  );
}
