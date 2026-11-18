import { Check } from 'lucide-react';
import { Fragment, type ReactNode } from 'react';
import type { Task } from '../api';
import { Face, GathrFace } from '../components/Face';
import { WORD } from '../components/marks';
import { Outcome } from '../components/Outcome';
import { StateMark } from '../components/StateMark';
import { asker, gathrs } from './model';
import { ended, grouped, said } from './rows';
import './task-list.css';

/** A task's state on its line, in the line's colour: Failed red, a question waiting on you blue (0071's shapes). */
export function TaskState({ task, tasks }: { task: Task; tasks: Task[] }) {
  const s = said(task, tasks);
  if ('done' in s) return null;
  if ('you' in s) return <span className="task-state you">Waiting</span>;
  if ('word' in s) return <span className="task-state">{s.word}</span>;
  if ('mark' in s)
    return (
      <span className="task-state">
        <StateMark state={s.mark} size={12} />
        {WORD[s.mark]}
      </span>
    );
  return <Outcome out={s.icon} className="task-state" />;
}

/** One quiet line, "·" between its pieces: the same pieces in the same order wherever it is drawn. */
export function Line({ pieces, className }: { pieces: ReactNode[]; className?: string }) {
  const shown = pieces.filter((p) => p !== null && p !== undefined && p !== false && p !== '');
  return (
    <span className={className ? `task-line ${className}` : 'task-line'}>
      {shown.map((p, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: a line's pieces keep their order
        <Fragment key={i}>
          {i > 0 && <span aria-hidden="true">·</span>}
          {typeof p === 'string' ? <span>{p}</span> : p}
        </Fragment>
      ))}
    </span>
  );
}

/** Who a row is about: a question's asker, any other task's owner. */
function whoOf(task: Task) {
  return task.kind === 'person' ? (gathrs(task) ? null : task.giver) : task.owner;
}

interface RowProps {
  task: Task;
  /** The chat's tasks: a waiting task names those it comes after that aren't done. */
  tasks: Task[];
  name: (agent: string | null) => string;
  /** In an agent's tab every row is its own: no face and no name. */
  bare?: boolean;
  onOpen: (task: number) => void;
}

/** One task in a list (the Tasks tab, an agent's tab): a plain face, its what, and "#24 · Bo · Blocked". */
export function TaskRow({ task, tasks, name, bare, onOpen }: RowProps) {
  const who = whoOf(task);
  const label = task.kind === 'person' ? asker(task, name) : name(task.owner);
  return (
    <button
      type="button"
      className="task-row"
      data-task={task.number}
      data-ended={ended(task) || undefined}
      data-bare={bare || undefined}
      onClick={() => onOpen(task.number)}
    >
      {!bare && (who ? <Face agent={{ id: who, name: name(who) }} size={24} /> : <GathrFace size={24} />)}
      <span className="task-row-text">
        <span className="task-row-what">
          <span>{task.what}</span>
          {task.state === 'done' && <Check aria-hidden="true" />}
        </span>
        <Line
          className="task-row-line"
          pieces={[
            `#${task.number}`,
            !bare && label,
            task.look_into && 'Look into',
            task.state !== 'done' && <TaskState key="state" task={task} tasks={tasks} />,
          ]}
        />
      </span>
    </button>
  );
}

/** Rows in a list; each opens its task. */
export function TaskRows(props: Omit<RowProps, 'task'> & { shown: Task[] }) {
  const { shown, ...row } = props;
  return (
    <ul className="task-rows">
      {shown.map((t) => (
        <li key={t.number}>
          <TaskRow task={t} {...row} />
        </li>
      ))}
    </ul>
  );
}

/** The chat's tasks in three groups, For you, Open and Closed, each under a muted heading with its count. */
export function TaskList({ tasks, name, onOpen }: Omit<RowProps, 'task' | 'bare'>) {
  return (
    <div className="task-list">
      {grouped(tasks).map((g) => (
        <section key={g.title} aria-label={g.title}>
          <h3>
            {g.title}
            <span>{g.tasks.length}</span>
          </h3>
          <TaskRows shown={g.tasks} tasks={tasks} name={name} onOpen={onOpen} />
        </section>
      ))}
    </div>
  );
}
