import { Fragment } from 'react';
import type { Member, Task } from '../api';
import { began, roleName } from '../chat/team';
import { Face, GathrFace, YouFace } from '../components/Face';
import { asker, type Draft, gathrs, type Refusal } from './model';
import { Question } from './Question';
import { asQuestion } from './rows';
import { Top } from './TaskView';

interface Props {
  task: Task;
  tasks: Task[];
  members: Member[];
  name: (agent: string | null) => string;
  draft: Draft;
  onDraft: (draft: Draft) => void;
  onSend: (draft: Draft) => Promise<Refusal | null>;
  onReference: (ref: string) => void;
  onOpenFile: (task: number, file: number) => void;
  onBack: () => void;
}

/** What the person sent, as their message reads: a Discuss's words, or the answer. */
function sent(task: Task): string[] {
  const a = task.answer as
    | { pick?: string[] | null; yes?: boolean | null; done?: boolean | null; text?: string | null }
    | null
    | undefined;
  if (task.state === 'held') return [task.answer?.discussed?.at(-1)?.text ?? ''].filter(Boolean);
  if (task.state !== 'done' || !a) return [];
  const picked = a.pick?.length
    ? a.pick.join(', ')
    : a.yes != null
      ? a.yes
        ? 'Yes'
        : 'No'
      : a.done
        ? 'Done'
        : '';
  return [picked, a.text ?? ''].filter(Boolean);
}

/**
 * A question opened from the list or from the chat's Open task (the tasks mock-up): the asker's message with the
 * question block in its bubble, answered here, then the person's own words once sent.
 */
export function QuestionView({
  task,
  tasks,
  members,
  name,
  draft,
  onDraft,
  onSend,
  onReference,
  onOpenFile,
  onBack,
}: Props) {
  const q = asQuestion(task, tasks);
  const own = gathrs(task);
  const role = members.length > 1 && members.find((m) => m.agent === task.giver)?.role;
  const said = sent(task);
  return (
    <section className="tasks" aria-label={`Task #${task.number}`}>
      <div className="tasks-scroll">
        <div className="tasks-page said-list">
          <Top number={task.number} onBack={onBack} />
          <div className="said">
            {own || !task.giver ? (
              <GathrFace size={36} />
            ) : (
              <Face agent={{ id: task.giver, name: name(task.giver) }} size={36} />
            )}
            <div className="said-main">
              <div className="said-line">
                <b>{asker(task, name)}</b>
                <span className="task-line">
                  {[role && roleName[role], began(task.created_us), q.for_agent && `for ${name(q.for_agent)}`]
                    .filter(Boolean)
                    .map((p, i) => (
                      <Fragment key={String(p)}>
                        {i > 0 && <span aria-hidden="true">·</span>}
                        <span>{p}</span>
                      </Fragment>
                    ))}
                </span>
              </div>
              <div className="said-bubble" data-open={task.state === 'ready' || undefined}>
                {q.why && <p>{q.why}</p>}
                {q.files.length > 0 && (
                  <p className="said-files">
                    {q.files.map((f, i) => (
                      <a
                        key={f.name}
                        href={`/api/tasks/${task.number}/files/${i}`}
                        onClick={(e) => {
                          e.preventDefault();
                          onOpenFile(task.number, i);
                        }}
                      >
                        {f.name}
                      </a>
                    ))}
                  </p>
                )}
                <Question
                  task={q}
                  draft={draft}
                  onDraft={onDraft}
                  name={name}
                  onSend={onSend}
                  onReference={onReference}
                />
              </div>
            </div>
          </div>
          {said.length > 0 && (
            <div className="said you">
              <YouFace size={36} />
              <div className="said-main">
                <div className="said-line">
                  <b>You</b>
                </div>
                <div className="said-bubble">
                  {said.map((p) => (
                    <p key={p}>{p}</p>
                  ))}
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
