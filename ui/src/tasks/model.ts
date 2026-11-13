import type { PersonAnswer, PersonTask, Task } from '../api';

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

/** Whether Send would answer it. Notes alone answer any question in words: "none of these" is an answer (G152). */
export function ready(task: PersonTask, draft: Draft | undefined): boolean {
  if (!draft) return false;
  if (task.form === 'text') return !!draft.values[0]?.trim();
  return draft.values.length > 0 || !!draft.notes.trim();
}

/** The staged draft as the service takes it; notes go with a pick as its text. */
export function toAnswer(task: PersonTask, draft: Draft): PersonAnswer {
  const notes = draft.notes.trim() ? { text: draft.notes.trim() } : {};
  if (task.form === 'text') return { task: task.number, text: draft.values[0].trim() };
  if (draft.values.length === 0) return { task: task.number, text: draft.notes.trim() };
  if (task.form === 'yesno') return { task: task.number, yes: draft.values[0] === 'Yes', ...notes };
  if (task.form === 'done') return { task: task.number, done: true, ...notes };
  return { task: task.number, pick: draft.values, ...notes };
}

const said = { done: 'Done', left: 'Left', blocked: 'Blocked' };

/** What a task's owner last reported, in a line, or that it has reported nothing (0047). */
export function reported(task: Task): string | null {
  const report = task.report;
  if (report) {
    const words = [said[report.how], report.note].filter(Boolean).join(': ');
    return report.see ? `${words} · ${report.see}` : words;
  }
  if (task.asked) return task.state === 'blocked' ? 'No report. Asked once' : 'No report yet. Asked once';
  return task.state === 'working' || task.state === 'held' ? 'No report yet' : null;
}

const short = (commit: string) => commit.slice(0, 7);

/** The commit a review is of, and where its PR is now once that has moved past it (G42). */
export function reviewedAt(task: Task): string | null {
  if (task.kind !== 'review' || !task.reviewed) return null;
  const at = short(task.reviewed);
  if (!task.outdated) return at;
  return task.pr && task.pr_head
    ? `${at}. Out of date: #${task.pr} is at ${short(task.pr_head)}`
    : `${at}. Out of date`;
}

/** How long nothing has moved a stale task (F11): "Idle 4 days". */
export function idle(sinceUs: number, now = Date.now()): string {
  const days = Math.max(1, Math.floor((now - sinceUs / 1000) / 86_400_000));
  return `Idle ${days} ${days === 1 ? 'day' : 'days'}`;
}
