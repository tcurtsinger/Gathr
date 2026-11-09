import { useState } from 'react';
import type { Failure } from '../api';
import type { Draft, PersonTask } from './model';
import type { Answer } from './TasksTab';

/**
 * The person's open questions across chats, with what the person has staged for each and where
 * each chat's Tasks tab stands. Drafts are kept by question, so they survive tabs and chats.
 */
export function usePersonTasks() {
  // Filled from the service once its shape lands (Cass, task 1164).
  const [tasks, setTasks] = useState<PersonTask[]>([]);
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [selected, setSelected] = useState<Record<string, string>>({});
  const [open, setOpen] = useState<Record<string, boolean>>({});

  const send = async (answers: Answer[]): Promise<Failure[]> => {
    const sent = new Set(answers.map((a) => a.task.id));
    setTasks((list) => list.filter((t) => !sent.has(t.id)));
    return [];
  };

  return {
    tasks,
    setTasks,
    drafts,
    draft: (id: string, d: Draft) => setDrafts((all) => ({ ...all, [id]: d })),
    selected,
    select: (chat: string, id: string) => setSelected((s) => ({ ...s, [chat]: id })),
    open,
    setOpen: (chat: string, shown: boolean) => setOpen((o) => ({ ...o, [chat]: shown })),
    send,
  };
}
