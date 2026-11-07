import { useCallback, useEffect, useRef, useState } from 'react';
import { api, type Failure } from '../api';
import { type LiveEvent, listen } from '../live';
import { report, timeToPaint } from '../report';
import { answered, apply, type ChatState, empty, fromHistory } from './state';

export interface SendFailure {
  text: string;
  failure: Failure;
}

/** One chat: its history, kept current from the live socket, and what the person can do in it. */
export function useChat(chat: string) {
  const [state, setState] = useState<ChatState>(empty);
  const [loaded, setLoaded] = useState(false);
  const [loadFailure, setLoadFailure] = useState<Failure | null>(null);
  const [sendFailure, setSendFailure] = useState<SendFailure | null>(null);
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
    setSendFailure(null);
    load();
    return listen((event) => {
      if (event.type === 'lagged') return load();
      if (event.chat !== chat) return;
      if (pending.current) pending.current.push(event);
      else setState((s) => apply(s, event));
    });
  }, [chat, load]);

  const send = useCallback(
    async (text: string) => {
      setSendFailure(null);
      try {
        await api.send(chat, text);
        return true;
      } catch (e) {
        setSendFailure({ text, failure: e as Failure });
        return false;
      }
    },
    [chat],
  );

  const stop = useCallback(
    () => api.stop(chat).catch((e: Failure) => setSendFailure({ text: '', failure: e })),
    [chat],
  );

  const approve = useCallback(
    (request: string, allow: boolean) => {
      setState((s) => answered(s, request));
      api.approve(chat, request, allow).catch((e: Failure) => setSendFailure({ text: '', failure: e }));
    },
    [chat],
  );

  return { state, loaded, loadFailure, sendFailure, send, stop, approve, reload: load };
}
