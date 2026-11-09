import { useEffect, useRef, useState } from 'react';
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

  // Counts every newer list (live, or a Send), so a read started before one never overwrites it.
  const version = useRef(0);
  useEffect(() => {
    const load = () => {
      const at = version.current;
      api.personTasks().then((list) => version.current === at && setTasks(list), onFailure);
    };
    load();
    return listen((event) => {
      if (event.type === 'person_tasks') {
        version.current++;
        setTasks(event.tasks);
      }
      if (event.type === 'lagged') load();
    });
  }, [onFailure]);

  const send = async (ready: PersonTask[]): Promise<Refusal[]> => {
    try {
      const { answered, refused } = await api.answer(ready.map((t) => toAnswer(t, drafts[t.number])));
      const closed = new Set(answered.map((t) => t.number));
      version.current++;
      setTasks((list) => list.filter((t) => !closed.has(t.number)));
      setDrafts((all) => Object.fromEntries(Object.entries(all).filter(([n]) => !closed.has(Number(n)))));
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
