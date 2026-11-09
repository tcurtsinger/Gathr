import { useEffect, useRef, useState } from 'react';
import type { Agent, Chat } from '../api';
import { Composer } from '../components/Composer';
import { Notice } from '../components/Notice';
import { report } from '../report';
import { AgentPicker } from './AgentPicker';
import type { SendFailure } from './messages';
import { running } from './state';
import { type Actions, Thread } from './Thread';
import { type Unsent, useChat } from './useChat';
import './chat.css';

interface Props {
  chat: Chat;
  agents: Agent[];
  /** Opens Settings > Agents at this agent. */
  onEditAgent: (agent: string) => void;
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

export function ChatView(props: Props) {
  const { chat, agents, onEditAgent, draft, onDraft, unsent, onUnsent, openSettings, onNamed } = props;
  const { state, loaded, loadFailure, failed, send, stop, approve, reload } = useChat(
    chat.id,
    unsent,
    onUnsent,
  );
  const turn = running(state);
  const input = useRef<HTMLTextAreaElement>(null);
  const [picked, setPicked] = useState<string | null>(null);
  // The one picked here, else the agent of the chat's last turn, else the first; a removed one is skipped.
  const last = Object.values(state.turns).findLast((t) => t.agent)?.agent;
  const agent = [picked, last].find((id) => agents.some((a) => a.id === id)) ?? agents[0]?.id;

  useFirstTokenReport(state.partial);

  // A failed send is kept for Retry, so the composer is free for the next message at once.
  const sendDraft = () => {
    const first = state.messages.length === 0;
    onDraft('');
    send(draft, agent).then(() => first && onNamed());
  };

  const actions: Actions = {
    diagnostics: (t) => openSettings('diagnostics', t),
    signIn: () => openSettings('engines'),
    // To the agent it was sent to (none if it was sent to none), never the one picked now:
    // if that agent was removed, the service refuses and the failure shows with its reference.
    retry: (t) => {
      const asked = state.messages.find((m) => m.turn === t && m.role === 'user');
      const turn = state.turns[t];
      if (asked && turn) send(asked.body as string, turn.agent ?? undefined);
    },
    resend: (f) => send(f.text, f.agent, f),
    carryOn: () => {
      onDraft('Continue');
      input.current?.focus();
    },
    approve,
  };

  return (
    <main className="chat" aria-label={chat.title}>
      <header className="topbar">
        <h1>{chat.title}</h1>
      </header>
      <Thread
        state={state}
        agents={agents}
        unsent={failed}
        running={!!turn}
        actions={actions}
        empty={loaded && state.messages.length === 0 && failed.length === 0}
        header={
          loadFailure && (
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
          )
        }
      >
        <Composer
          draft={draft}
          onDraft={onDraft}
          onSend={sendDraft}
          onStop={turn ? stop : undefined}
          inputRef={input}
          tools={
            agent && (
              <AgentPicker
                agents={agents}
                selected={agent}
                onSelect={setPicked}
                onEdit={() => onEditAgent(agent)}
              />
            )
          }
        />
      </Thread>
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
