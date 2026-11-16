import type { PersonAnswer, PersonTask, ReviewNote, Task, TaskField } from '../api';
import { began, list } from '../chat/team';

/** Who asks a question no agent gave: Gathr itself, as the far end's stop does (0054). */
const GATHR = 'Gathr';

/**
 * Who gave a task, by name. A question for the person that no agent gave is Gathr's own; any other
 * task with no giver is the person's.
 */
export function asker(task: Pick<Task, 'kind' | 'giver'>, name: (agent: string | null) => string): string {
  return task.kind === 'person' && task.giver === null ? GATHR : name(task.giver);
}

/** What the person has staged for a question: kept through Back, Next, tabs and chats until Send. */
export interface Draft {
  values: string[];
  notes: string;
  /** Discuss is on: the notes go to the asker alone, and the picks wait, kept, until it's off. */
  discuss?: boolean;
}

/** The rows to pick from. Labels are unique within a question, so a label is its id. */
export function options(
  task: PersonTask,
  name: (agent: string | null) => string,
): { id: string; label: string; description?: string }[] {
  if (task.form === 'yesno')
    return [
      { id: 'Yes', label: 'Yes' },
      { id: 'No', label: 'No' },
    ];
  if (task.form === 'done') return [{ id: 'Done', label: 'Done' }];
  return task.choices.map((c, i) => ({
    id: c.label,
    label: c.label,
    // The asker's pick comes first (DESIGN.md); Gathr's own question has no pick to recommend.
    description:
      task.form === 'choice' && task.giver !== null && i === 0
        ? [`${asker(task, name)}'s pick`, c.detail].filter(Boolean).join(' · ')
        : c.detail,
  }));
}

/**
 * Whether Send would answer it. Notes alone answer an agent's question in words: "none of these" is an
 * answer (G152). Gathr's own question takes one of its choices: it can't act on words.
 */
export function ready(task: PersonTask, draft: Draft | undefined): boolean {
  if (!draft) return false;
  if (draft.discuss) return !!draft.notes.trim();
  if (task.form === 'text') return !!draft.values[0]?.trim();
  if (task.giver === null) return draft.values.length === 1;
  return draft.values.length > 0 || !!draft.notes.trim();
}

/** The staged draft as the service takes it; notes go with a pick as its text. */
export function toAnswer(task: PersonTask, draft: Draft): PersonAnswer {
  const notes = draft.notes.trim() ? { text: draft.notes.trim() } : {};
  if (draft.discuss) return { task: task.number, discuss: true, text: draft.notes.trim() };
  if (task.form === 'text') return { task: task.number, text: draft.values[0].trim() };
  if (draft.values.length === 0) return { task: task.number, text: draft.notes.trim() };
  if (task.form === 'yesno') return { task: task.number, yes: draft.values[0] === 'Yes', ...notes };
  if (task.form === 'done') return { task: task.number, done: true, ...notes };
  return { task: task.number, pick: draft.values, ...notes };
}

const said = { done: 'Done', left: 'Left', blocked: 'Blocked' };

/** What a task's owner last reported, in a line, or that it has reported nothing (0047). */
export function reported(task: Task): string | null {
  // The person's question has no owner to report: held, it is only waiting off the list (0065).
  if (task.kind === 'person') return null;
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

const kinds = { wrong: 'Wrong', unasked: 'Unasked', suggestion: 'Suggestion' };

/** A review's note in a line: "Unasked · src/cache.rs: what it is" (0053). */
export const noteLine = (n: ReviewNote) => `${kinds[n.kind]} · ${n.file ? `${n.file}: ` : ''}${n.says}`;

/** A review's notes, one line each. */
export function noteLines(task: Task): string[] {
  return (task.notes ?? []).map(noteLine);
}

/** A task's size: "src/export.ts, about 120 lines". */
export function sized(task: Pick<Task, 'size' | 'lines'>): string | null {
  return task.size ? `${task.size}, about ${task.lines} lines` : null;
}

/**
 * A work task's brief as its view shows it, label and words, each with its field; `after` already named
 * ("#41", "part 1"). A plan's part shows the same rows, as its task will carry them (0062 H5.4).
 */
export function briefRows(
  task: Pick<Task, 'done' | 'proof' | 'where' | 'size' | 'lines'>,
  after: string[],
): [label: string, words: string | null | undefined, field: TaskField][] {
  return [
    ['Done when', task.done, 'done'],
    ['Proof', task.proof, 'proof'],
    ['Where', task.where, 'where'],
    ['Size', sized(task), 'size'],
    ['After', after.join(', '), 'after'],
  ];
}

const fieldWords: Record<TaskField, string> = {
  what: 'title',
  why: 'why',
  done: 'done when',
  proof: 'proof',
  where: 'where',
  size: 'size',
  after: 'after',
};

/** Its giver's last edit: "Title and after · 14:02". */
export function edited(task: Task): string | null {
  if (!task.edited?.length || !task.edited_us) return null;
  const words = list(task.edited.map((f) => fieldWords[f]));
  return `${words[0].toUpperCase()}${words.slice(1)} · ${began(task.edited_us)}`;
}

/** How long nothing has moved a stale task (F11): "Idle 4 days". */
export function idle(sinceUs: number, now = Date.now()): string {
  const days = Math.max(1, Math.floor((now - sinceUs / 1000) / 86_400_000));
  return `Idle ${days} ${days === 1 ? 'day' : 'days'}`;
}
