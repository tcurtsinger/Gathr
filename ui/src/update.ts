import { useCallback, useEffect, useState } from 'react';
import { type AppState, api, type Failure } from './api';
import { listen } from './live';
import { useNewestWins } from './newest';

/**
 * Gathr's own version and update (0032), live. Read when it loads and again whenever the live socket
 * missed events (a hand-over to the update drops it), then followed as the service sends it. Of a
 * read and a live state, only the newest started shows.
 */
export function useAppUpdate(onFailure: (f: Failure) => void) {
  const [app, setApp] = useState<AppState | null>(null);
  // From the click until the service answers: a second click would only be refused.
  const [asking, setAsking] = useState(false);
  const newest = useNewestWins();
  const load = useCallback(() => {
    newest(api.app()).then(setApp, onFailure);
  }, [newest, onFailure]);
  useEffect(() => {
    load();
    return listen((event) => {
      if (event.type === 'app') newest(Promise.resolve(event)).then(setApp);
      if (event.type === 'lagged') load();
    });
  }, [load, newest]);

  /** Update: the service stops what runs, switches to the ready release, and its agents carry on. */
  const update = useCallback(() => {
    setAsking(true);
    api
      .updateApp()
      .then(load, onFailure)
      .finally(() => setAsking(false));
  }, [load, onFailure]);
  const check = useCallback(() => {
    api.checkApp().then(load, onFailure);
  }, [load, onFailure]);

  return { app, asking, update, check };
}

/** The version Update would install, while one is ready and none is being installed. */
export const readyVersion = (app: AppState | null) =>
  app?.state === 'ready' ? (app.update?.version ?? null) : null;
