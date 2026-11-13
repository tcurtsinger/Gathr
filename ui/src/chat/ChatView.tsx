import { Folder } from 'lucide-react';
import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Button } from '@/components/ui/button';
import {
  type Agent,
  api,
  type Chat,
  type Failure,
  type Member,
  type PauseScope,
  type Pauses,
  type Project,
} from '../api';
import { Composer } from '../components/Composer';
import { FailureNotice } from '../components/Notice';
import { report } from '../report';
import { AgentPanel, blocksOf, type Kept, noneKept } from './AgentPanel';
import { AgentPicker, type Recipient } from './AgentPicker';
import { ApprovalDock } from './ApprovalDock';
import { ChatMenu } from './ChatMenu';
import { useLatest, useTarget } from './found';
import { activity, type Glance, glances } from './glance';
import { type Custom, type SendFailure, toThread } from './messages';
import { type PauseAction, type Run, runOf } from './pauses';
import { Summary } from './Summary';
import { running } from './state';
import { type Actions, Thread, useStable } from './Thread';
import { roleName } from './team';
import { type Unsent, useChat } from './useChat';
import { useWaiting } from './useWaiting';
import { WaitingLine } from './Waiting';
import { Working, working } from './Working';
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
  /** Opens an agent's task in the workspace's Tasks tab (owner's pick A, room task 70). */
  onOpenTask?: (task: number) => void;
  /** The agent whose tab the workspace has, by id, and whether that tab is the one shown. */
  agentOpen?: { agent: string; shown: boolean };
  /** Opens an agent's panel in the workspace, from Summary or its initial in the timeline. */
  onOpenAgent?: (agent: string) => void;
  /** Closes the workspace: it lies over the composer when Message is pressed in a narrow window. */
  onCloseWorkspace?: () => void;
  /**
   * The workspace, beside the chat. It is given the open agent's name and the place for its panel, which
   * only the chat can draw. The chat asks again only when this function or the open agent changes, never
   * for a streamed token, so the workspace's own tabs aren't drawn again with each one.
   */
  workspace?: (agent?: { name: string; panel: ReactNode }) => ReactNode;
  /** A search hit's message to open at (none: a title, the latest), and when it was picked; done once shown. */
  found?: { message?: string; at: number };
  onFound?: () => void;
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

  const cache = useRef(new Map());
  const timeline = useMemo(() => toThread(state, agents, failed, cache.current), [state, agents, failed]);
  const team = !fresh && state.members.length > 1;
  const before = useRef<Glance[]>([]);
  // What the chat waits on, as the service answers it; a pause changes it with no live message.
  const waiting = useWaiting(fresh || archived ? undefined : chat.id, pauses);
  const crew = glances(state, activity(timeline.items), waiting, before.current);
  before.current = crew;
  // Summary, and what last opened or closed it when that wasn't its own button: focus goes back there.
  const [summary, setSummary] = useState<{ open: boolean; from?: HTMLElement }>({ open: false });
  const hit = useTarget(props.found, loaded, timeline, props.onFound);
  // An approval shown from the dock, until a search hit is opened after it.
  const { target, pick: setShown } = useLatest(hit);
  const show = (request: string) => {
    const asks = (m: (typeof timeline.items)[number]) =>
      (m.metadata?.custom as Custom | undefined)?.approvals?.some((a) => a.request === request);
    const id = timeline.items.find(asks)?.id;
    if (id) setShown({ id, card: request, at: performance.now() });
  };

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
    openTask: (task) => props.onOpenTask?.(task),
    // Only an agent of this chat has a panel: one that left it can still be named in the timeline.
    openAgent: (id) => {
      if (crew.some((g) => g.member.agent === id)) props.onOpenAgent?.(id);
    },
  };
  const stable = useStable(actions);
  const { agentOpen, workspace, onCloseWorkspace } = props;
  const opened = crew.find((g) => g.member.agent === agentOpen?.agent);
  const message = useCallback(
    (id: string) => {
      setPicked(id);
      if (matchMedia('(max-width: 1200px)').matches) onCloseWorkspace?.();
      input.current?.focus();
    },
    [onCloseWorkspace],
  );
  // The panel's place in the workspace: the same element whatever streams, so the workspace isn't asked
  // for again. The panel is drawn into it from here and stays there while another tab shows, keeping its
  // scroll and its opened rows; hidden, it is handed what it was last drawn from, so nothing redraws it.
  const [slot, setSlot] = useState<HTMLElement | null>(null);
  const name = opened?.member.name;
  const tab = useMemo(
    () => (name === undefined ? undefined : { name, panel: <div className="agent-slot" ref={setSlot} /> }),
    [name],
  );
  const beside = useMemo(() => workspace?.(tab), [workspace, tab]);
  // Each agent's compaction asked for here: its turn, and its status until the live turn says more.
  const [compacts, setCompacts] = useState<Record<string, { turn: string; status: string }>>({});
  const compact = useCallback(
    (agent: string) =>
      attempt('compact', () =>
        api
          .compact(chat.id, agent)
          .then(({ turn, status }) => setCompacts((c) => ({ ...c, [agent]: { turn, status } }))),
      ),
    [attempt, chat.id],
  );
  const asked = opened && compacts[opened.member.agent];
  const compaction = asked && (state.turns[asked.turn]?.status ?? asked.status);
  const kept = useRef(new Map<string, Kept>());
  const drawn = useRef<{ agent: string; panel: ReactNode }>(undefined);
  const panel = () => {
    if (!slot || !opened) return null;
    const { agent } = opened.member;
    if (agentOpen?.shown || drawn.current?.agent !== agent) {
      const mine = kept.current.get(agent) ?? noneKept();
      kept.current.set(agent, mine);
      drawn.current = {
        agent,
        panel: (
          <AgentPanel
            key={agent}
            glance={opened}
            tasks={state.tasks}
            members={state.members}
            {...blocksOf(timeline.items, agent, mine)}
            actions={stable}
            onMessage={!archived && recipients.some((a) => a.id === agent) ? message : undefined}
            onCompact={archived ? undefined : compact}
            compaction={compaction === 'queued' || compaction === 'running' ? compaction : undefined}
          />
        ),
      };
    }
    return createPortal(drawn.current.panel, slot);
  };

  return (
    <>
      <main className="chat" aria-label={chat.title}>
        <header className="topbar">
          <div className="topbar-title">
            <h1>{chat.title}</h1>
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
                className="head-button"
                align="start"
              />
            )}
            {archived && <small className="topbar-state">Archived</small>}
          </div>
          <div className="topbar-actions">
            {team && (
              <Summary
                state={state}
                team={crew}
                open={summary.open}
                onOpen={(open) => setSummary((s) => ({ ...s, open }))}
                onOwnButton={() => setSummary((s) => ({ open: s.open }))}
                from={summary.from}
                pauses={pauses}
                onPause={act}
                onOpenAgent={actions.openAgent}
              />
            )}
            {headerEnd}
          </div>
        </header>
        {props.notice}
        <div className="chat-body">
          <Thread
            messages={timeline.items}
            running={!!turn}
            actions={actions}
            target={target}
            onTarget={props.onFound}
            empty={loaded && state.messages.length === 0 && failed.length === 0}
            after={
              !archived && (
                <>
                  <RunLine run={run} members={state.members} />
                  {run.state === 'running' && (
                    <Working
                      state={state}
                      summary={summary.open}
                      onOpen={team ? (from) => setSummary((s) => ({ open: !s.open, from })) : undefined}
                    />
                  )}
                  {/* In Working's place while none runs: what the chat waits on. A queued turn
                      that hasn't started is one of the things it says. */}
                  {run.state === 'running' && !working(state) && <WaitingLine waiting={waiting} />}
                </>
              )
            }
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
              <>
                <ApprovalDock
                  approvals={state.approvals}
                  name={(t) => members.find((m) => m.agent === state.turns[t]?.agent)?.name ?? 'An agent'}
                  onShow={(a) => show(a.request)}
                  onAnswer={approve}
                />
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
              </>
            )}
          </Thread>
        </div>
      </main>
      {beside}
      {panel()}
    </>
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
