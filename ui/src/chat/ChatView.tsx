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
  type Named,
  type PauseScope,
  type Pauses,
  type Project,
  type Task,
} from '../api';
import { Composer } from '../components/Composer';
import { FaceCards } from '../components/FaceCard';
import { first as firstPick, type Picked } from '../components/mention';
import { FailureNotice } from '../components/Notice';
import { report } from '../report';
import { AgentPanel, blocksOf, type Kept, noneKept } from './AgentPanel';
import { ApprovalDock } from './ApprovalDock';
import { ChatMenu } from './ChatMenu';
import { MarksContext, useChatMarks } from './faceMarks';
import { useLatest, useTarget } from './found';
import { activity, type Glance, glances } from './glance';
import { type Custom, type SendFailure, toThread } from './messages';
import { type PauseAction, type Run, runOf } from './pauses';
import { addressed, nameable, unnamed } from './recipient';
import { StatusLine } from './StatusLine';
import { Summary } from './Summary';
import { running } from './state';
import { type Actions, Thread, useStable } from './Thread';
import { type Unsent, useChat } from './useChat';
import { useWaiting } from './useWaiting';
import './chat.css';

interface Props {
  chat: Chat;
  agents: Agent[];
  /** Removed agents, by name: their old messages still say who wrote them (0069). */
  removed?: Named[];
  draft: string;
  /** The names picked from the "@" list in the draft: only these count (C24). */
  picks?: Picked[];
  onDraft: (text: string, picks?: Picked[]) => void;
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
  /** At the top line's end: the panel's button. */
  headerEnd?: ReactNode;
  /** At the chat's top, whatever is scrolled to: the app's latest failure. */
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
  /** Opens an agent's panel in the workspace, from Summary or its face in the timeline. */
  onOpenAgent?: (agent: string) => void;
  /** Opens a plan's tab in the workspace at a version, from its chip or a plan line (0062). */
  onOpenPlan?: (plan: number, version: number) => void;
  /**
   * The workspace, beside the chat. It is given the open agent's name and the place for its panel, which
   * only the chat can draw, and the chat's tasks and members for its Tasks tab. The chat asks again only when
   * one of these changes, never for a streamed token, so the workspace's own tabs aren't drawn again with each one.
   */
  workspace?: (
    agent: { id: string; name: string; panel: ReactNode } | undefined,
    work: { tasks: Task[]; members: Member[] },
  ) => ReactNode;
  /** A search hit's message to open at (none: a title, the latest), and when it was picked; done once shown. */
  found?: { message?: string; at: number };
  onFound?: () => void;
}

const NONE: Named[] = [];
const NO_PICKS: Picked[] = [];

export function ChatView(props: Props) {
  const { chat, agents, draft, picks = NO_PICKS, onDraft, unsent, onUnsent, openSettings, onNamed } = props;
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
  // A new chat isn't made yet: its team is its project's, if it has one.
  const fresh = !chat.id;
  const members = fresh ? (projects.find((p) => p.id === chat.project)?.members ?? []) : state.members;
  const last = Object.values(state.turns).findLast((t) => t.agent)?.agent;

  const cache = useRef(new Map());
  const { removed = NONE } = props;
  const named = useMemo(() => [...agents, ...removed], [agents, removed]);
  const timeline = useMemo(() => toThread(state, named, failed, cache.current), [state, named, failed]);
  const team = !fresh && state.members.length > 1;
  const before = useRef<Glance[]>([]);
  // What the chat waits on, as the service answers it; a pause changes it with no live message.
  const waiting = useWaiting(fresh || archived ? undefined : chat.id, pauses);
  const crew = glances(state, activity(timeline.items), waiting, before.current);
  before.current = crew;
  const marks = useChatMarks(crew, timeline.items, archived);
  // Who the message box can name, and who gets a message that names nobody (C24: no picker).
  const room = { members, agents, project: !!chat.project, fresh, last, marks: marks.still };
  const people = nameable(room);
  const fallback = unnamed(people, members, last);
  const to = addressed(picks, people, fallback, fresh);
  const [summary, setSummary] = useState(false);
  const hit = useTarget(props.found, loaded, timeline, props.onFound);
  // An approval shown from the dock, until a search hit is opened after it.
  const { target, pick: setShown } = useLatest(hit);
  const member = (id: string) => ({ id, name: members.find((m) => m.agent === id)?.name ?? 'An agent' });
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
    onDraft('', []);
    if (fresh) return onStart(draft, to.to?.id);
    send(draft, to.to?.id).then(() => first && onNamed());
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
      onDraft('Continue', []);
      input.current?.focus();
    },
    approve,
    openTask: (task) => props.onOpenTask?.(task),
    openPlan: (plan, version) => props.onOpenPlan?.(plan, version),
    // Only an agent of this chat has a panel: one that left it can still be named in the timeline.
    openAgent: (id) => {
      if (crew.some((g) => g.member.agent === id)) props.onOpenAgent?.(id);
    },
  };
  const stable = useStable(actions);
  const { agentOpen, workspace } = props;
  const opened = crew.find((g) => g.member.agent === agentOpen?.agent);
  // Message on a panel: the agent picked first in the box, unless it already is.
  const typed = useRef({ draft, picks, people, onDraft });
  typed.current = { draft, picks, people, onDraft };
  const message = useCallback((id: string) => {
    const { draft, picks, people, onDraft } = typed.current;
    const who = people.find((p) => p.id === id);
    if (who && firstPick(picks)?.id !== id) {
      const moved = picks.map((p) => ({ ...p, at: p.at + who.name.length + 2 }));
      onDraft(`@${who.name} ${draft}`, [{ at: 0, id, name: who.name }, ...moved]);
    }
    requestAnimationFrame(() => {
      const box = input.current;
      box?.focus();
      box?.setSelectionRange(box.value.length, box.value.length);
    });
  }, []);
  // The panel's place in the workspace: the same element whatever streams, so the workspace isn't asked
  // for again. The panel is drawn into it from here and stays there while another tab shows, keeping its
  // scroll and its opened rows; hidden, it is handed what it was last drawn from, so nothing redraws it.
  const [slot, setSlot] = useState<HTMLElement | null>(null);
  const id = opened?.member.agent;
  const name = opened?.member.name;
  const tab = useMemo(
    () =>
      id === undefined || name === undefined
        ? undefined
        : { id, name, panel: <div className="agent-slot" ref={setSlot} /> },
    [id, name],
  );
  const { tasks: chatTasks, members: chatMembers } = state;
  const beside = useMemo(
    () => workspace?.(tab, { tasks: chatTasks, members: chatMembers }),
    [workspace, tab, chatTasks, chatMembers],
  );
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
            onMessage={!archived && people.some((p) => p.id === agent) ? message : undefined}
            onCompact={archived ? undefined : compact}
            compaction={compaction === 'queued' || compaction === 'running' ? compaction : undefined}
          />
        ),
      };
    }
    return createPortal(drawn.current.panel, slot);
  };

  return (
    <MarksContext value={marks}>
      <FaceCards>
        {/* The window's top line (0070), on the shell above the frame: the app's grid puts it there. */}
        <header className="topline">
          <div className="topline-title">
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
            {archived && <small className="topline-state">Archived</small>}
          </div>
          <div className="topline-actions">
            {team && (
              <Summary
                state={state}
                team={crew}
                open={summary}
                onOpen={setSummary}
                pauses={pauses}
                onPause={act}
                onOpenAgent={actions.openAgent}
              />
            )}
            {headerEnd}
          </div>
        </header>
        <main className="chat" aria-label={chat.title}>
          {props.notice}
          <div className="chat-body">
            <Thread
              messages={timeline.items}
              running={!!turn}
              actions={actions}
              target={target}
              onTarget={props.onFound}
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
                  {restoreFailure && (
                    <FailureNotice failure={restoreFailure} onReference={actions.reference} />
                  )}
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
                    plans={state.plans}
                    asker={(t) => member(state.turns[t]?.agent ?? '')}
                    lead={member}
                    onShow={(a) => show(a.request)}
                    onAnswer={approve}
                    onOpenPlan={(plan, version) => props.onOpenPlan?.(plan, version)}
                  />
                  {!fresh && <StatusLine crew={crew} waiting={waiting} onOpenAgent={actions.openAgent} />}
                  <Composer
                    draft={draft}
                    picks={picks}
                    onDraft={onDraft}
                    onSend={sendDraft}
                    onStop={turn ? () => act('stop', thisChat) : undefined}
                    inputRef={input}
                    hint={fallback ? `Message ${fallback.name}` : 'Message'}
                    people={people}
                    to={to.to && { id: to.to.id, name: to.to.name, joins: to.joins }}
                  />
                </>
              )}
            </Thread>
          </div>
        </main>
        {beside}
        {panel()}
      </FaceCards>
    </MarksContext>
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
