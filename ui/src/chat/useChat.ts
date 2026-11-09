import { startTransition, useCallback, useEffect, useRef, useState } from 'react';
import { api, type Failure } from '../api';
import { type LiveEvent, listen } from '../live';
import { report, timeToPaint } from '../report';
import type { SendFailure } from './messages';
import { answered, apply, type ChatState, empty, fromLoad } from './state';

/** Changes the chat's messages that never reached the service; the app keeps them across switches. */
export type Unsent = (update: (list: SendFailure[]) => SendFailure[]) => void;

/** One chat: its history, kept current from the live socket, and what the person can do in it. */
export function useChat(chat: string, unsent: SendFailure[], onUnsent: Unsent) {
  const [state, setState] = useState<ChatState>(empty);
  const [loaded, setLoaded] = useState(false);
  const [loadFailure, setLoadFailure] = useState<Failure | null>(null);
  // A Stop or an answer that failed: nothing to resend, so it lives only while the chat is open.
  const [actionFailure, setActionFailure] = useState<SendFailure | null>(null);
  const pending = useRef<LiveEvent[] | null>(null);
  const openedAt = useRef(0);

  const load = useCallback(() => {
    pending.current = [];
    openedAt.current = performance.now();
    Promise.all([api.history(chat), api.tasks(chat)]).then(
      ([history, work]) => {
        const held = pending.current ?? [];
        pending.current = null;
        setState(fromLoad(history, work, held));
        setLoaded(true);
        setLoadFailure(null);
        timeToPaint('gathr.ui.chat_open', openedAt.current, { messages: history.messages.length });
      },
      (failure: Failure) => {
        pending.current = null;
        setLoadFailure(failure);
        report({ name: 'gathr.ui.chat_open', error: failure.error, attrs: { ref: failure.ref } });
      },
    );
  }, [chat]);

  useEffect(() => {
    setState(empty);
    setLoaded(false);
    setActionFailure(null);
    // A new chat the service hasn't made yet has nothing to load.
    if (!chat) return setLoaded(true);
    load();
    // Each socket message is its own task, so five replies streaming at once would draw the chat
    // many times a frame: what arrives before the next frame is drawn once.
    let queued: LiveEvent[] = [];
    let frame = 0;
    const flush = () => {
      // A reply that ends redraws all of itself, so replies that end together end a frame apart.
      let ended = false;
      const cut = queued.findIndex((e) => {
        if (e.type !== 'turn' || e.status === 'running' || e.status === 'queued') return false;
        if (ended) return true;
        ended = true;
        return false;
      });
      const events = cut < 0 ? queued : queued.slice(0, cut);
      queued = cut < 0 ? [] : queued.slice(cut);
      frame = queued.length ? requestAnimationFrame(flush) : 0;
      // Interruptible: a long redraw yields to the next frame instead of holding it.
      startTransition(() => setState((s) => events.reduce(apply, s)));
    };
    const stop = listen((event) => {
      if (event.type === 'lagged') {
        queued = [];
        return load();
      }
      if (!('chat' in event) || event.chat !== chat) return;
      if (pending.current) return pending.current.push(event);
      queued.push(event);
      frame ||= requestAnimationFrame(flush);
    });
    return () => {
      stop();
      cancelAnimationFrame(frame);
    };
  }, [chat, load]);

  const send = useCallback(
    async (text: string, agent?: string, retrying?: SendFailure) => {
      try {
        await api.send(chat, text, agent);
        if (retrying) onUnsent((list) => list.filter((f) => f.id !== retrying.id));
      } catch (e) {
        const failed = { id: retrying?.id ?? crypto.randomUUID(), text, agent, failure: e as Failure };
        onUnsent((list) =>
          retrying ? list.map((f) => (f.id === failed.id ? failed : f)) : [...list, failed],
        );
      }
    },
    [chat, onUnsent],
  );

  const stop = useCallback(
    () => api.stop(chat).catch((e: Failure) => setActionFailure({ id: 'stop', text: '', failure: e })),
    [chat],
  );

  const approve = useCallback(
    (request: string, allow: boolean) => {
      setState((s) => answered(s, request));
      api
        .approve(chat, request, allow)
        .catch((e: Failure) => setActionFailure({ id: 'answer', text: '', failure: e }));
    },
    [chat],
  );

  const failed = actionFailure ? [...unsent, actionFailure] : unsent;
  return { state, loaded, loadFailure, failed, send, stop, approve, reload: load };
}
