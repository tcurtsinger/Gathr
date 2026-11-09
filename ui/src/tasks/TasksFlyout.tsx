import { ListTodo, X } from 'lucide-react';
import { Popover } from 'radix-ui';
import { useState } from 'react';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import type { PersonTask, StaleTask } from '../api';
import { idle } from './model';
import './flyout.css';

interface Props {
  tasks: PersonTask[];
  /** Agents' stale tasks across chats: grouped with the person's stale questions (F11). */
  stale: StaleTask[];
  name: (agent: string | null) => string;
  onOpen: (task: PersonTask | StaleTask) => void;
}

/**
 * The person's open questions across every chat (DESIGN.md "Navigation and workspace"): the
 * prototype's Radix flyout beside navigation, so its own tooltips draw above it. Picking one opens
 * its chat's Tasks tab at that question.
 */
export function TasksFlyout({ tasks, stale, name, onOpen }: Props) {
  const [open, setOpen] = useState(false);
  const questions = tasks.filter((t) => !t.stale_us);
  // Each once: the person's own first, as they're the ones only the person can move.
  const quiet = [...tasks.filter((t) => t.stale_us), ...stale];
  const row = (t: PersonTask | StaleTask) => (
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
        {t.stale_us ? (t.owner ? name(t.owner) : 'For you') : name(t.giver)} · {t.chat_title}
      </small>
      {t.stale_us && (
        <small>
          {idle(t.stale_us)}
          {t.owner && ` · ${t.state}`}
        </small>
      )}
    </button>
  );
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
            <section aria-labelledby="flyout-questions">
              <h3 id="flyout-questions">
                Questions<span className="nav-count">{questions.length || ''}</span>
              </h3>
              {questions.length ? questions.map(row) : <p className="flyout-empty">No questions</p>}
            </section>
            <section aria-labelledby="flyout-stale">
              <h3 id="flyout-stale">
                Stale<span className="nav-count">{quiet.length || ''}</span>
              </h3>
              {quiet.length ? quiet.map(row) : <p className="flyout-empty">No stale tasks</p>}
            </section>
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
