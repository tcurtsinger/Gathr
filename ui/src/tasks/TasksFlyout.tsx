import { ListTodo } from 'lucide-react';
import { useState } from 'react';
import type { PersonTask, StaleTask } from '../api';
import { NavFlyout } from '../components/NavFlyout';
import { idle } from './model';

interface Props {
  tasks: PersonTask[];
  /** Agents' stale tasks across chats: grouped with the person's stale questions (F11). */
  stale: StaleTask[];
  name: (agent: string | null) => string;
  onOpen: (task: PersonTask | StaleTask) => void;
}

/** The person's open questions across every chat. Picking one opens its chat's Tasks tab at that question. */
export function TasksFlyout({ tasks, stale, name, onOpen }: Props) {
  const [open, setOpen] = useState(false);
  const questions = tasks.filter((t) => !t.stale_us);
  // Each once: the person's own first, as they're the ones only the person can move.
  const quiet = [...tasks.filter((t) => t.stale_us), ...stale];
  const row = (t: PersonTask | StaleTask) => (
    <button
      type="button"
      key={t.number}
      className="flyout-row"
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
    <NavFlyout
      title="Tasks"
      open={open}
      onOpenChange={setOpen}
      focus={(panel) => panel.querySelector<HTMLElement>('.flyout-row')}
      trigger={
        <>
          <ListTodo aria-hidden="true" />
          Tasks
          {tasks.length > 0 && <span className="nav-count">{tasks.length}</span>}
        </>
      }
    >
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
    </NavFlyout>
  );
}
