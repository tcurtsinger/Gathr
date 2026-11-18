import { useCallback, useEffect, useRef, useState } from 'react';
import { api, type Failure, type PersonTask, type StaleTask } from '../api';
import { listen } from '../live';
import { useNewestWins } from '../newest';
import { type Draft, type Refusal, toAnswer } from './model';

/**
 * The person's open questions across chats, live, and agents' stale tasks (F11), with what the person has staged
 * for each and whether each chat's panel is open. Drafts are kept by question, so they survive tabs and chats.
 */
export function usePersonTasks(onFailure: (f: Failure) => void) {
  const [tasks, setTasks] = useState<PersonTask[]>([]);
  const [stale, setStale] = useState<StaleTask[]>([]);
  const [drafts, setDrafts] = useState<Record<number, Draft>>({});
  const [open, setOpen] = useState<Record<string, boolean>>({});

  // Reads, live lists and a Send's closes: a read started before a newer one never overwrites it.
  const newest = useNewestWins();
  const show = useCallback(
    (list: Promise<{ tasks: PersonTask[]; stale: StaleTask[] }>) =>
      newest(list).then((l) => {
        setTasks(l.tasks);
        setStale(l.stale);
      }),
    [newest],
  );
  // Ends when its answer comes, shown or not, so a Cancel that reads after it never waits forever.
  const load = useCallback(() => {
    const list = api.personTasks();
    show(list).catch(onFailure);
    return list.then(
      () => {},
      () => {},
    );
  }, [show, onFailure]);
  useEffect(() => {
    load();
    return listen((event) => {
      if (event.type === 'person_tasks') show(Promise.resolve(event));
      if (event.type === 'lagged') load();
    });
  }, [load, show]);

  // The chat and the Tasks view both offer a question (0073): while one Send of it is out, another sends nothing.
  const sending = useRef(new Set<number>());
  // One question at a time (0065): its draft goes as it is now, so a pick that answers at once needn't wait for it.
  const send = async (task: PersonTask, draft: Draft): Promise<Refusal | null> => {
    if (sending.current.has(task.number)) return null;
    sending.current.add(task.number);
    try {
      const { answered, refused } = await api.answer([toAnswer(task, draft)]);
      const closed = new Set(answered.map((t) => t.number));
      newest(Promise.resolve()).then(() => setTasks((list) => list.filter((t) => !closed.has(t.number))));
      setDrafts((all) => Object.fromEntries(Object.entries(all).filter(([n]) => !closed.has(Number(n)))));
      return refused[0] ?? null;
    } catch (e) {
      const f = e as Failure;
      return { task: task.number, why: f.why ?? f.error, ref: f.ref };
    } finally {
      sending.current.delete(task.number);
    }
  };

  return {
    tasks,
    stale,
    drafts,
    draft: (task: number, d: Draft) => setDrafts((all) => ({ ...all, [task]: d })),
    open,
    setOpen: (chat: string, shown: boolean) => setOpen((o) => ({ ...o, [chat]: shown })),
    send,
    /** Cancels or restarts an agent's task, then reads the list again, live updates or not. */
    settle: (task: number, action: 'cancel' | 'restart') =>
      (action === 'cancel' ? api.cancelTask : api.restartTask)(task).then(load),
  };
}
