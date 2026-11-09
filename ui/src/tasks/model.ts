import type { PersonAnswer, PersonTask } from '../api';

/** What the person has staged for a question: kept through Back, Next, tabs and chats until Send. */
export interface Draft {
  values: string[];
  notes: string;
}

/** The rows to pick from. Labels are unique within a question, so a label is its id. */
export function options(task: PersonTask): { id: string; label: string; description?: string }[] {
  if (task.form === 'yesno')
    return [
      { id: 'Yes', label: 'Yes' },
      { id: 'No', label: 'No' },
    ];
  if (task.form === 'done') return [{ id: 'Done', label: 'Done' }];
  return task.choices.map((c, i) => ({
    id: c.label,
    label: c.label,
    // The asker's pick comes first (DESIGN.md).
    description:
      task.form === 'choice' && i === 0 ? ['Recommended', c.detail].filter(Boolean).join(' · ') : c.detail,
  }));
}

/** Whether Send would answer it. Notes alone never answer a pick (DESIGN.md "Human tasks"). */
export function ready(task: PersonTask, draft: Draft | undefined): boolean {
  if (!draft) return false;
  return task.form === 'text' ? !!draft.values[0]?.trim() : draft.values.length > 0;
}

/** The staged draft as the service takes it; notes go with a pick as its text. */
export function toAnswer(task: PersonTask, draft: Draft): PersonAnswer {
  const notes = draft.notes.trim() ? { text: draft.notes.trim() } : {};
  if (task.form === 'text') return { task: task.number, text: draft.values[0].trim() };
  if (task.form === 'yesno') return { task: task.number, yes: draft.values[0] === 'Yes', ...notes };
  if (task.form === 'done') return { task: task.number, done: true, ...notes };
  return { task: task.number, pick: draft.values, ...notes };
}
