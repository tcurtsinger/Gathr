/** A question the Supervisor put to the person (DESIGN.md "Human tasks"). */
export interface PersonTask {
  id: string;
  chat: string;
  chatTitle: string;
  /** The agent who asks: the chat's Supervisor. */
  agent: string;
  title: string;
  /** One pick, several, or the person's own words. Yes/no and Done are picks. */
  answer: 'one' | 'many' | 'text';
  options: { id: string; label: string; description?: string }[];
  file?: string;
}

/** What the person has staged for a task: kept through Back, Next, tabs and chats until Send. */
export interface Draft {
  values: string[];
  notes: string;
}

/** Whether Send would answer it. Notes alone never answer a pick. */
export function ready(task: PersonTask, draft: Draft | undefined): boolean {
  if (!draft) return false;
  return task.answer === 'text' ? !!draft.values[0]?.trim() : draft.values.length > 0;
}
