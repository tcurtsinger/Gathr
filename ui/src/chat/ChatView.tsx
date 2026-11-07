import { useEffect, useMemo, useRef } from 'react';
import type { Chat } from '../api';
import { Composer } from '../components/Composer';
import { Notice } from '../components/Notice';
import { report } from '../report';
import { rows, type SendFailure } from './rows';
import { running } from './state';
import { DiagnosticsLink, type NoticeActions, Transcript } from './Transcript';
import { type Unsent, useChat } from './useChat';
import './chat.css';

interface Props {
  chat: Chat;
  draft: string;
  onDraft: (text: string) => void;
  /** Opens Settings at a category; Diagnostics opens at the turn given. */
  openSettings: (page: 'engines' | 'diagnostics', turn?: string) => void;
  /** Messages that never reached the service, kept by the app while other chats are open. */
  unsent: SendFailure[];
  onUnsent: Unsent;
  /** The first message names the chat (the service does it), so the list shows the name. */
  onNamed: () => void;
}

export function ChatView({ chat, draft, onDraft, unsent, onUnsent, openSettings, onNamed }: Props) {
  const { state, loaded, loadFailure, failed, send, stop, approve, reload } = useChat(
    chat.id,
    unsent,
    onUnsent,
  );
  const list = useMemo(() => rows(state, failed), [state, failed]);
  const turn = running(state);
  const scroller = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const pinned = useRef(true);
  const ours = useRef(0);

  useFirstTokenReport(state.partial);

  // Keeps the newest text in view while it streams, and while off-screen rows settle from
  // their estimated height to their real one, unless the person scrolled up to read.
  useEffect(() => {
    const el = scroller.current;
    if (!el?.firstElementChild) return;
    const follow = new ResizeObserver(() => {
      if (!pinned.current) return;
      el.scrollTop = el.scrollHeight;
      ours.current = el.scrollTop;
    });
    follow.observe(el.firstElementChild);
    return () => follow.disconnect();
  }, []);

  const submit = (text: string, retrying?: SendFailure) => {
    pinned.current = true;
    return send(text, retrying);
  };
  // A failed send is kept for Retry, so the composer is free for the next message at once.
  const sendDraft = () => {
    const first = state.messages.length === 0;
    onDraft('');
    submit(draft).then(() => first && onNamed());
  };

  const actions: NoticeActions = {
    diagnostics: (t) => openSettings('diagnostics', t),
    signIn: () => openSettings('engines'),
    retry: (t) => {
      const asked = state.messages.find((m) => m.turn === t && m.role === 'user');
      if (asked) submit(asked.body as string);
    },
    resend: (f) => submit(f.text, f),
    carryOn: () => {
      onDraft('Continue');
      input.current?.focus();
    },
  };

  return (
    <main className="chat" aria-label={chat.title}>
      <header className="topbar">
        <h1>{chat.title}</h1>
        <DiagnosticsLink open={() => openSettings('diagnostics')} />
      </header>
      <div
        className="scroller"
        ref={scroller}
        onScroll={(e) => {
          const el = e.currentTarget;
          if (el.scrollTop === ours.current) return;
          pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
        }}
      >
        <div className="reading">
          {loadFailure && (
            <Notice
              danger
              title={loadFailure.error}
              reference={
                loadFailure.ref ? { ref: loadFailure.ref, open: () => actions.diagnostics() } : undefined
              }
              action={
                <button type="button" onClick={reload}>
                  Retry
                </button>
              }
            />
          )}
          {loaded && list.length === 0 && <p className="empty">No messages yet</p>}
          <Transcript rows={list} actions={actions} />
          {state.approvals.map((a) => (
            <Notice
              key={a.request}
              title={`Allow ${a.tool}?`}
              links={
                <button type="button" className="link" onClick={() => approve(a.request, false)}>
                  Deny
                </button>
              }
              action={
                <button type="button" onClick={() => approve(a.request, true)}>
                  Allow
                </button>
              }
            />
          ))}
        </div>
      </div>
      <div className="composer-area">
        <Composer
          draft={draft}
          onDraft={onDraft}
          onSend={sendDraft}
          onStop={turn ? stop : undefined}
          inputRef={input}
        />
      </div>
    </main>
  );
}

/** Reports when each turn's first streamed text is drawn, to set against the service's
 * own first-token time for that turn (docs/first-slice.md: within 300 ms). */
function useFirstTokenReport(partial: Record<string, string>) {
  const seen = useRef(new Set<string>());
  useEffect(() => {
    for (const turn of Object.keys(partial)) {
      if (seen.current.has(turn)) continue;
      seen.current.add(turn);
      requestAnimationFrame(() =>
        report({ name: 'gathr.ui.first_token', attrs: { turn, shown_ms: Date.now() } }),
      );
    }
  }, [partial]);
}
