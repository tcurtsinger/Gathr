import { useCallback, useEffect, useRef, useState } from 'react';
import { api, type Failure, type PauseScope, type Pauses } from '../api';
import { listen } from '../live';
import { useNewestWins } from '../newest';

/** What the person can do to a scope (0010): Restart is the resume of a stopped one. */
export type PauseAction = 'pause' | 'resume' | 'stop';

/** How a chat runs, as its header, quiet line and composer show it. */
export interface Run {
  state: 'running' | 'pausing' | 'paused' | 'stopped';
  /** The agent that paused it; null for the person. */
  by: string | null;
  /** The pause that holds it: its own, or the whole app's. */
  scope: PauseScope | null;
}

const none: Pauses = { pauses: [], usage: [] };

/**
 * Every pause and usage hold, live: one list for the whole app, as the service keeps it. Of reads,
 * actions' answers and live lists, only the newest started shows. An action that another list
 * overlapped reads the list again: which of them the service applied last can't be told from the
 * window. A refusal rejects with its reference.
 */
export function usePauses(onFailure: (f: Failure) => void) {
  const [pauses, setPauses] = useState<Pauses>(none);
  const newest = useNewestWins();
  // Counts every read, action and live list as it starts.
  const started = useRef(0);
  const load = useCallback(() => {
    started.current++;
    newest(api.pauses()).then(setPauses, onFailure);
  }, [newest, onFailure]);
  useEffect(() => {
    load();
    return listen((event) => {
      if (event.type === 'pauses') {
        started.current++;
        newest(Promise.resolve(event)).then(setPauses);
      }
      if (event.type === 'lagged') load();
    });
  }, [load, newest]);

  const act = useCallback(
    async (action: PauseAction, scope: PauseScope) => {
      const at = ++started.current;
      const answer = api[action](scope);
      // Shown only if it's newest; a refusal rejects below either way.
      newest(answer).then(setPauses, () => {});
      await answer;
      if (started.current !== at) load();
    },
    [newest, load],
  );

  return { pauses, act };
}

/** The chat's own pause, else the app's; else it runs. */
export function runOf({ pauses }: Pauses, chat: string): Run {
  const p =
    pauses.find((p) => p.scope === 'chat' && p.target === chat) ?? pauses.find((p) => p.scope === 'app');
  if (!p) return { state: 'running', by: null, scope: null };
  const scope: PauseScope = p.scope === 'app' ? { scope: 'app' } : { scope: 'chat', target: chat };
  return { state: p.state, by: p.by, scope };
}

/** An agent's own pause, and when its account's usage comes back while it's out. */
export function agentPause({ pauses, usage }: Pauses, agent: string) {
  return {
    paused: pauses.some((p) => p.scope === 'agent' && p.target === agent),
    // Held turns start 30 s after the reset (0010), so a hold that has just passed still holds.
    usageUntil:
      usage.find((u) => u.agents.includes(agent) && u.until_us + 30e6 > Date.now() * 1000)?.until_us ?? null,
  };
}
