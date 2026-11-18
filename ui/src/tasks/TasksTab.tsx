import type { Member, Task } from '../api';
import type { Draft, Refusal } from './model';
import { QuestionView } from './QuestionView';
import { TaskList } from './TaskList';
import { TaskView } from './TaskView';
import './tasks.css';

interface Props {
  /** The chat's tasks, as its state holds them: every one, cancelled and done ones too. */
  tasks: Task[];
  members: Member[];
  /** The task shown, from the list, the chat's Open task or the Tasks flyout; none shows the list. */
  opened?: number;
  onOpen: (task: number | undefined) => void;
  drafts: Record<number, Draft>;
  onDraft: (task: number, draft: Draft) => void;
  /** An agent's name by its id. */
  name: (agent: string | null) => string;
  onOpenFile: (task: Task, file: number) => void;
  /** Answers one question, or sends its Discuss: none once the service took it, else why not. */
  onAnswer: (task: Task, draft: Draft) => Promise<Refusal | null>;
  onSettle: (task: Task, action: 'cancel' | 'restart') => Promise<unknown>;
  onReference: (ref: string) => void;
  /** The chat's project: a task with a PR shows Gathr's checks run there. */
  project?: string | null;
}

/**
 * The chat's Tasks tab (the tasks mock-up, New): its tasks in one list, For you, Open and Closed; a row opens its
 * task, and a question opens as itself, answered in place, one at a time (0065).
 */
export function TasksTab(props: Props) {
  const { tasks, opened, onOpen, drafts, onDraft, name, onReference } = props;
  const task = tasks.find((t) => t.number === opened);
  const back = () => onOpen(undefined);
  if (task?.kind === 'person')
    return (
      <QuestionView
        key={task.number}
        task={task}
        tasks={tasks}
        members={props.members}
        name={name}
        draft={drafts[task.number] ?? { values: [], notes: '' }}
        onDraft={(d) => onDraft(task.number, d)}
        onSend={(d) => props.onAnswer(task, d)}
        onReference={onReference}
        onOpenFile={(_, file) => props.onOpenFile(task, file)}
        onBack={back}
      />
    );
  if (task)
    return (
      <TaskView
        key={task.number}
        task={task}
        tasks={tasks}
        project={props.project}
        name={name}
        onSettle={props.onSettle}
        onReference={onReference}
        onBack={back}
      />
    );
  if (!tasks.length) return <p className="tasks-empty">No open tasks</p>;
  return (
    <section className="tasks" aria-label="Tasks">
      <div className="tasks-scroll tasks-listed">
        <TaskList tasks={tasks} name={name} onOpen={onOpen} />
      </div>
    </section>
  );
}
