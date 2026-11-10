import { Folder } from 'lucide-react';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import type { Agent, Chat, Failure, Member, PauseScope, Pauses, Project } from '../api';
import { Composer } from '../components/Composer';
import { FailureNotice } from '../components/Notice';
import { report } from '../report';
import { AgentPicker, type Recipient } from './AgentPicker';
import { ChatMenu } from './ChatMenu';
import type { SendFailure } from './messages';
import { type PauseAction, type Run, runOf } from './pauses';
import { Summary } from './Summary';
import { running } from './state';
import { type Actions, Thread } from './Thread';
import { roleName } from './team';
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
  projects: Project[];
  /** A new chat's project, picked before its first message; '' for none. */
  onProject: (project: string) => void;
  onNewProject: () => void;
  /** Sends a new chat's first message: the service makes the chat then. */
  onStart: (text: string, agent?: string) => void;
  /** At the header's end: the workspace's opener. */
  headerEnd?: ReactNode;
  /** Under the header, whatever is scrolled to: the app's latest failure. */
  notice?: ReactNode;
  /** Every pause, and what the person can do to one (the app keeps them for every chat). */
  pauses: Pauses;
  onPause: (action: PauseAction, scope: PauseScope) => Promise<void>;
  /** Archive and Delete ask first (the app's Confirm); Restore doesn't, and never restarts its work. */
  onArchive?: (opener: HTMLElement | null) => void;
  onDelete?: (opener: HTMLElement | null) => void;
  onRestore?: () => void;
  restoreFailure?: Failure;
  onReference?: (ref: string) => void;
}

export function ChatView(props: Props) {
  const { chat, agents, onEditAgent, draft, onDraft, unsent, onUnsent, openSettings, onNamed } = props;
  const { projects, onProject, onNewProject, onStart, headerEnd, pauses, onPause } = props;
  const archived = !!chat.archived;
  const { onRestore, restoreFailure } = props;
  const { state, loaded, loadFailure, failed, send, attempt, approve, reload } = useChat(
    chat.id,
    unsent,
    onUnsent,
  );
  const turn = running(state);
  const run = runOf(pauses, chat.id);
  const act = (action: PauseAction, scope: PauseScope) => attempt(action, () => onPause(action, scope));
  const thisChat: PauseScope = { scope: 'chat', target: chat.id };
  const input = useRef<HTMLTextAreaElement>(null);
  const [picked, setPicked] = useState<string | null>(null);
  // A new chat isn't made yet: its team is its project's, if it has one.
  const fresh = !chat.id;
  const members = fresh ? (projects.find((p) => p.id === chat.project)?.members ?? []) : state.members;
  const recipients = recipientsOf(members, agents, !!chat.project);
  // The one picked here, else the chat's Supervisor, else the agent of its last turn, else the first;
  // a removed one is skipped.
  const lead = members.find((m) => m.role === 'supervisor')?.agent;
  const last = Object.values(state.turns).findLast((t) => t.agent)?.agent;
  const agent = [picked, lead, last].find((id) => recipients.some((a) => a.id === id)) ?? recipients[0]?.id;

  useFirstTokenReport(state.partial);
  // A failed send is kept for Retry, so the composer is free for the next message at once.
  const sendDraft = () => {
    const first = state.messages.length === 0;
    onDraft('');
    if (fresh) return onStart(draft, agent);
    send(draft, agent).then(() => first && onNamed());
  };

  const actions: Actions = {
    diagnostics: (t) => openSettings('diagnostics', t),
    reference: (ref) => (props.onReference ? props.onReference(ref) : openSettings('diagnostics')),
    signIn: () => openSettings('engines'),
    // To the agent it was sent to (none if it was sent to none), never the one picked now:
    // if that agent was removed, the service refuses and the failure shows with its reference.
    retry: (t) => {
      const asked = state.messages.find((m) => m.turn === t && m.role === 'user');
      const turn = state.turns[t];
      if (asked && turn) send(asked.body as string, turn.agent ?? undefined);
    },
    resend: (f) => {
      if (!fresh) return send(f.text, f.agent, f);
      onUnsent((list) => list.filter((x) => x.id !== f.id));
      onStart(f.text, f.agent);
    },
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
        {archived && <small className="topbar-state">Archived</small>}
        <div className="topbar-actions">
          {!fresh && (
            <ChatMenu
              pause={{
                state: run.state,
                all: run.scope?.scope === 'app',
                onPause: () => act('pause', thisChat),
                onResume: () => act('resume', run.scope ?? thisChat),
              }}
              archived={archived}
              onArchive={props.onArchive}
              onRestore={onRestore}
              onDelete={props.onDelete}
            />
          )}
          {state.members.length > 1 && <Summary state={state} pauses={pauses} run={run} onPause={act} />}
          {headerEnd}
        </div>
      </header>
      {props.notice}
      <Thread
        state={state}
        agents={agents}
        unsent={failed}
        running={!!turn}
        actions={actions}
        empty={loaded && state.messages.length === 0 && failed.length === 0}
        after={!archived && <RunLine run={run} members={state.members} />}
        header={
          loadFailure && (
            <FailureNotice
              failure={loadFailure}
              onReference={actions.reference}
              action={
                <Button type="button" variant="outline" onClick={reload}>
                  Retry
                </Button>
              }
            />
          )
        }
      >
        {fresh && (
          <label className="project-pick">
            <Folder aria-hidden="true" />
            <select
              aria-label="Project"
              value={chat.project ?? ''}
              onChange={(e) => (e.target.value === 'new' ? onNewProject() : onProject(e.target.value))}
            >
              <option value="">No project</option>
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
              <option value="new">New project</option>
            </select>
          </label>
        )}
        {archived ? (
          <>
            {restoreFailure && <FailureNotice failure={restoreFailure} onReference={actions.reference} />}
            <div className="archived-line">
              Archived
              <Button size="sm" variant="outline" onClick={onRestore}>
                Restore
              </Button>
            </div>
          </>
        ) : (
          <Composer
            draft={draft}
            onDraft={onDraft}
            onSend={sendDraft}
            onStop={turn ? () => act('stop', thisChat) : undefined}
            inputRef={input}
            tools={
              agent && (
                <AgentPicker
                  agents={recipients}
                  selected={agent}
                  onSelect={setPicked}
                  onEdit={() => onEditAgent(agent)}
                />
              )
            }
          />
        )}
      </Thread>
    </main>
  );
}

/** The quiet line after the latest content while the chat isn't running (the prototype's run note). */
function RunLine({ run, members }: { run: Run; members: Member[] }) {
  if (run.state === 'running') return null;
  const who = run.by ? (members.find((m) => m.agent === run.by)?.name ?? 'An agent') : 'You';
  return (
    <p className="run-line" role="status">
      {run.state === 'pausing' ? 'Pausing…' : `${who} ${run.state}`}
    </p>
  );
}

const MOST = 5;

/**
 * Who a message can go to (0028): the chat's Supervisor by name first, then its other agents. A chat
 * without a project can also take any other agent, up to five: sending to one adds it.
 */
function recipientsOf(members: Member[], agents: Agent[], project: boolean): Recipient[] {
  const sorted = [...members].sort(
    (a, b) => Number(b.role === 'supervisor') - Number(a.role === 'supervisor'),
  );
  const inChat = sorted
    .filter((m) => agents.some((a) => a.id === m.agent))
    .map((m) => ({ id: m.agent, name: m.name, meta: members.length > 1 ? roleName[m.role] : undefined }));
  if (project || members.length >= MOST) return inChat;
  return [...inChat, ...agents.filter((a) => !members.some((m) => m.agent === a.id))];
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
