import { createContext, useContext } from 'react';
import type { Task } from '../api';
import type { Draft, Refusal } from '../tasks/model';
import { Question } from '../tasks/Question';
import { asQuestion } from '../tasks/rows';
import { useActions } from './Thread';

/** What the chat answers its questions with: the drafts and the Send the Tasks view uses too (0073). */
export interface Questions {
  /** The chat's tasks. */
  tasks: Task[];
  drafts: Record<number, Draft>;
  onDraft: (task: number, draft: Draft) => void;
  onSend: (task: Task, draft: Draft) => Promise<Refusal | null>;
  name: (agent: string | null) => string;
}

export const QuestionsContext = createContext<Questions | null>(null);

const NONE: Draft = { values: [], notes: '' };

/**
 * A question in its asker's bubble, answered there with the one question block (0065, 0073). It is talked over with
 * Reply in its thread, not the Discuss switch, which stays in the Tasks view (0076).
 */
export function Asked({ task: number }: { task: number }) {
  const questions = useContext(QuestionsContext);
  const actions = useActions();
  const task = questions?.tasks.find((t) => t.number === number);
  if (!questions || !task) return null;
  return (
    <Question
      task={asQuestion(task, questions.tasks)}
      draft={questions.drafts[number] ?? NONE}
      onDraft={(d) => questions.onDraft(number, d)}
      name={questions.name}
      onSend={(d) => questions.onSend(task, d)}
      onReference={actions.reference}
      discuss={false}
    />
  );
}
