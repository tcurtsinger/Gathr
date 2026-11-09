import { useEffect, useState } from 'react';
import { api, type Failure, type PersonTask } from '../api';
import { listen } from '../live';
import { type Draft, toAnswer } from './model';
import type { Refusal } from './TasksTab';

/**
 * The person's open questions across chats, live, with what the person has staged for each and where
 * each chat's Tasks tab stands. Drafts are kept by question, so they survive tabs and chats.
 */
export function usePersonTasks(onFailure: (f: Failure) => void) {
  const [tasks, setTasks] = useState<PersonTask[]>([]);
  const [drafts, setDrafts] = useState<Record<number, Draft>>({});
  const [selected, setSelected] = useState<Record<string, number>>({});
  const [open, setOpen] = useState<Record<string, boolean>>({});

  useEffect(() => {
    const load = () => api.personTasks().then(setTasks, onFailure);
    load();
    return listen((event) => {
      if (event.type === 'person_tasks') setTasks(event.tasks);
      if (event.type === 'lagged') load();
    });
  }, [onFailure]);

  const send = async (ready: PersonTask[]): Promise<Refusal[]> => {
    try {
      const { answered, refused } = await api.answer(ready.map((t) => toAnswer(t, drafts[t.number])));
      setTasks((list) => list.filter((t) => !answered.includes(t.number)));
      setDrafts((all) =>
        Object.fromEntries(Object.entries(all).filter(([n]) => !answered.includes(Number(n)))),
      );
      return refused;
    } catch (e) {
      const f = e as Failure;
      return ready.map((t) => ({ task: t.number, why: f.why ?? f.error, ref: f.ref }));
    }
  };

  return {
    tasks,
    drafts,
    draft: (task: number, d: Draft) => setDrafts((all) => ({ ...all, [task]: d })),
    selected,
    select: (chat: string, task: number) => setSelected((s) => ({ ...s, [chat]: task })),
    open,
    setOpen: (chat: string, shown: boolean) => setOpen((o) => ({ ...o, [chat]: shown })),
    send,
  };
}
