import { ListTodo, X } from 'lucide-react';
import { Popover } from 'radix-ui';
import { useState } from 'react';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import type { PersonTask } from '../api';
import './flyout.css';

interface Props {
  tasks: PersonTask[];
  name: (agent: string | null) => string;
  onOpen: (task: PersonTask) => void;
}

/**
 * The person's open questions across every chat (DESIGN.md "Navigation and workspace"): the
 * prototype's Radix flyout beside navigation, so its own tooltips draw above it. Picking one opens
 * its chat's Tasks tab at that question.
 */
export function TasksFlyout({ tasks, name, onOpen }: Props) {
  const [open, setOpen] = useState(false);
  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button type="button" className="navbutton">
          <ListTodo aria-hidden="true" />
          Tasks
          {tasks.length > 0 && <span className="nav-count">{tasks.length}</span>}
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
          aria-label="Tasks"
          onOpenAutoFocus={(e) => {
            e.preventDefault();
            const panel = e.currentTarget as HTMLElement;
            (panel.querySelector<HTMLElement>('.flyout-task') ?? panel.querySelector('button'))?.focus();
          }}
        >
          <header>
            <h2>Tasks</h2>
            <TooltipIconButton tooltip="Close Tasks" onClick={() => setOpen(false)}>
              <X />
            </TooltipIconButton>
          </header>
          <div className="flyout-body">
            {tasks.length === 0 && <p className="flyout-empty">No tasks</p>}
            {tasks.map((t) => (
              <button
                type="button"
                key={t.number}
                className="flyout-task"
                onClick={() => {
                  setOpen(false);
                  onOpen(t);
                }}
              >
                <span>{t.what}</span>
                <small>
                  {name(t.giver)} · {t.chat_title}
                </small>
              </button>
            ))}
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
