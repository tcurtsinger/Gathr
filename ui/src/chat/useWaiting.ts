import { useEffect, useState } from 'react';
import { api, type Waiting } from '../api';
import { listen } from '../live';

/**
 * What the chat waits on (0050), as the service answers it: asked when the chat opens, and again each
 * time the service says the chat's answer may have changed, when Gathr's own update moves, and after
 * the socket dropped. `follows` is what else the answer depends on that the window learns without a
 * live message (the pauses, after an action's own answer): a new value asks again. One request at a
 * time, so answers never land out of order: a change during one asks once more after it.
 */
export function useWaiting(chat: string | undefined, follows?: unknown): Waiting | undefined {
  const [answer, setAnswer] = useState<{ chat: string; waiting: Waiting }>();
  // biome-ignore lint/correctness/useExhaustiveDependencies: `follows` asks again when it changes.
  useEffect(() => {
    if (!chat) return;
    let over = false;
    let busy = false;
    let again = false;
    const load = () => {
      if (busy) {
        again = true;
        return;
      }
      busy = true;
      api
        .waiting(chat)
        // A failed ask keeps the last answer: the next change asks again.
        .then(
          (waiting) => !over && Array.isArray(waiting.lines) && setAnswer({ chat, waiting }),
          () => {},
        )
        .finally(() => {
          busy = false;
          if (again && !over) {
            again = false;
            load();
          }
        });
    };
    load();
    const stop = listen((event) => {
      const mine = event.type === 'waiting' && event.chat === chat;
      if (mine || event.type === 'app' || event.type === 'lagged') load();
    });
    return () => {
      over = true;
      stop();
    };
  }, [chat, follows]);
  return answer && answer.chat === chat ? answer.waiting : undefined;
}
