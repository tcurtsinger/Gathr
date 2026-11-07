import { useCallback, useEffect, useRef, useState } from 'react';
import { api, type Failure } from '../api';
import { type LiveEvent, listen } from '../live';
import { report, timeToPaint } from '../report';
import type { SendFailure } from './rows';
import { answered, apply, type ChatState, empty, fromHistory } from './state';

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
    api.history(chat).then(
      (history) => {
        const held = pending.current ?? [];
        pending.current = null;
        setState(held.reduce(apply, fromHistory(history)));
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
    load();
    return listen((event) => {
      if (event.type === 'lagged') return load();
      if (event.chat !== chat) return;
      if (pending.current) pending.current.push(event);
      else setState((s) => apply(s, event));
    });
  }, [chat, load]);

  const send = useCallback(
    async (text: string, retrying?: SendFailure) => {
      try {
        await api.send(chat, text);
        if (retrying) onUnsent((list) => list.filter((f) => f.id !== retrying.id));
      } catch (e) {
        const failed = { id: retrying?.id ?? crypto.randomUUID(), text, failure: e as Failure };
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
