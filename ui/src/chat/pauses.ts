import { useCallback, useEffect, useRef, useState } from 'react';
import { api, type Failure, type PauseScope, type Pauses } from '../api';
import { listen } from '../live';

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
 * Every pause and usage hold, live: one list for the whole app, as the service keeps it. An action's
 * answer is the list then, unless a newer one came first; a refusal rejects with its reference.
 */
export function usePauses(onFailure: (f: Failure) => void) {
  const [pauses, setPauses] = useState<Pauses>(none);
  // Counts every newer list, so a read or an answer started before one never overwrites it.
  const version = useRef(0);
  useEffect(() => {
    const load = () => {
      const at = version.current;
      api.pauses().then((p) => version.current === at && setPauses(p), onFailure);
    };
    load();
    return listen((event) => {
      if (event.type === 'pauses') {
        version.current++;
        setPauses({ pauses: event.pauses, usage: event.usage });
      }
      if (event.type === 'lagged') load();
    });
  }, [onFailure]);

  const act = useCallback(async (action: PauseAction, scope: PauseScope) => {
    const at = version.current;
    const answer = await api[action](scope);
    if (version.current !== at) return;
    version.current++;
    setPauses(answer);
  }, []);

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
