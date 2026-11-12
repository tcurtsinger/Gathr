import {
  AssistantRuntimeProvider,
  MessagePrimitive,
  type ThreadMessageLike,
  ThreadPrimitive,
  useAuiState,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { ArrowDown } from 'lucide-react';
import { createContext, type ReactNode, useContext, useEffect, useMemo, useRef } from 'react';
import { ApprovalCard } from '@/components/assistant-ui/elements/approval-card';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import { Button } from '@/components/ui/button';
import { MarkdownText } from '../components/Markdown';
import { FailureNotice, Notice } from '../components/Notice';
import { asked } from './ApprovalDock';
import { useEdges } from './edges';
import { useFound } from './found';
import type { Custom, SendFailure, TurnNotice } from './messages';
import { useOlder } from './older';
import type { Approval } from './state';
import { Events, Exchange, FoundContext, Speaker, WorkRow } from './TurnParts';
import { roleName } from './team';
import './thread.css';

/** What a reply's notices and approvals can do. Stable for a chat, so replies don't redraw for it. */
export interface Actions {
  diagnostics: (turn?: string) => void;
  /** Opens an error's reference in Diagnostics. */
  reference: (ref: string) => void;
  signIn: () => void;
  retry: (turn: string) => void;
  /** Sends a message again that failed before any turn began. */
  resend: (failed: SendFailure) => void;
  carryOn: () => void;
  approve: (request: string, allow: boolean) => void;
  openTask: (task: number) => void;
  /** Opens an agent's panel in the workspace. */
  openAgent: (agent: string) => void;
}

const ActionsContext = createContext<Actions | null>(null);
const useActions = () => useContext(ActionsContext) as Actions;

interface Props {
  /** Its messages, from toThread. */
  messages: ThreadMessageLike[];
  running: boolean;
  actions: Actions;
  /** Shown above the messages: the chat's own load failure. */
  header?: ReactNode;
  /** Shown after the latest message: the quiet "You paused" line, or who is working. */
  after?: ReactNode;
  empty: boolean;
  /** The composer, kept below the messages as they scroll. */
  children?: ReactNode;
  /**
   * A search hit or a waiting approval to open at: centred and tinted, the view held there until the
   * person is back at the bottom.
   */
  target?: Target;
  /** The target is drawn and centred. */
  onTarget?: () => void;
}

/**
 * A thread message to open at, or with no `id` the latest one; `text` is the hit's own part of a reply, the
 * `nth` with that text; `card` is the approval asked in it, by its request. `at` is when it was picked.
 */
export interface Target {
  id?: string;
  text?: string;
  nth?: number;
  card?: string;
  at: number;
}

// One function for the life of the page: the runtime keeps what it converted only while its converter is
// the same one, and would convert every message again on each streamed token otherwise.
const asIs = (m: ThreadMessageLike) => m;

/** The chat's timeline, drawn by assistant-ui's thread over Gathr's own chat state (0030, 0037). */
export function Thread(props: Props) {
  const { running, actions, header, after, empty, children, target } = props;
  const viewport = useRef<HTMLDivElement>(null);
  const { messages, onScroll, reach } = useOlder(props.messages, viewport);
  const found = useFound(props.messages, viewport, reach, target, props.onTarget);
  useEdges(viewport);
  const runtime = useExternalStoreRuntime({
    messages,
    convertMessage: asIs,
    isRunning: running,
    // The composer is Gathr's own and sends through useChat, never through the runtime.
    onNew: async () => {},
  });
  const stable = useStable(actions);
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <ActionsContext value={stable}>
        <FoundContext value={found.text}>
          <ThreadPrimitive.Root className="thread">
            <ThreadPrimitive.Viewport
              ref={viewport}
              className="thread-viewport"
              scrollToBottomOnRunStart={!found.held}
              onScroll={() => {
                onScroll();
                found.onScroll();
              }}
            >
              <div className="thread-column" data-empty={empty || undefined}>
                {header}
                {empty && <h2 className="welcome">What are we working on?</h2>}
                <div className="thread-messages">
                  <ThreadPrimitive.Messages>{() => <ThreadMessage />}</ThreadPrimitive.Messages>
                </div>
                {after}
                <ThreadPrimitive.ViewportFooter className="thread-footer">
                  <ThreadPrimitive.ScrollToBottom asChild>
                    <TooltipIconButton
                      tooltip="Scroll to bottom"
                      variant="outline"
                      className="thread-bottom bg-background"
                    >
                      <ArrowDown />
                    </TooltipIconButton>
                  </ThreadPrimitive.ScrollToBottom>
                  {children}
                </ThreadPrimitive.ViewportFooter>
              </div>
            </ThreadPrimitive.Viewport>
          </ThreadPrimitive.Root>
        </FoundContext>
      </ActionsContext>
    </AssistantRuntimeProvider>
  );
}

/** Every message reads the actions, so they keep one identity and call the latest ones. */
export function useStable(actions: Actions): Actions {
  const latest = useRef(actions);
  latest.current = actions;
  return useMemo<Actions>(
    () => ({
      diagnostics: (turn) => latest.current.diagnostics(turn),
      reference: (ref) => latest.current.reference(ref),
      signIn: () => latest.current.signIn(),
      retry: (turn) => latest.current.retry(turn),
      resend: (failed) => latest.current.resend(failed),
      carryOn: () => latest.current.carryOn(),
      approve: (request, allow) => latest.current.approve(request, allow),
      openTask: (task) => latest.current.openTask(task),
      openAgent: (agent) => latest.current.openAgent(agent),
    }),
    [],
  );
}

/**
 * Blocks of the timeline on their own, drawn by the same components: the agent's panel lists an agent's
 * earlier ones this way.
 */
export function Blocks({ messages, actions }: { messages: ThreadMessageLike[]; actions: Actions }) {
  const runtime = useExternalStoreRuntime({
    messages,
    convertMessage: asIs,
    isRunning: false,
    onNew: async () => {},
  });
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <ActionsContext value={useStable(actions)}>
        <ThreadPrimitive.Root className="thread-messages">
          <ThreadPrimitive.Messages>{() => <ThreadMessage />}</ThreadPrimitive.Messages>
        </ThreadPrimitive.Root>
      </ActionsContext>
    </AssistantRuntimeProvider>
  );
}

function ThreadMessage() {
  const role = useAuiState((s) => s.message.role);
  return role === 'user' ? <UserMessage /> : role === 'system' ? <SystemItem /> : <AssistantMessage />;
}

const custom = (s: { message: { metadata: { custom: Record<string, unknown> } } }) =>
  s.message.metadata.custom as Custom;

/** Gathr's notes that belong to no turn's row. */
function SystemItem() {
  return <Events events={useAuiState(custom).events ?? []} onOpenTask={useActions().openTask} />;
}

function UserMessage() {
  const own = useAuiState(custom);
  const { unsent } = own;
  const actions = useActions();
  return (
    <MessagePrimitive.Root className="user-message">
      {/* A failed Stop or Pause sent no text: only its notice shows, with nothing to resend. */}
      {unsent?.text !== '' && (
        <div className="user-bubble">
          <MessagePrimitive.Parts />
        </div>
      )}
      {unsent && (
        <FailureNotice
          failure={unsent.failure}
          onReference={actions.reference}
          links={<DiagnosticsLink open={() => actions.diagnostics()} />}
          action={
            unsent.text && (
              <Button type="button" variant="outline" onClick={() => actions.resend(unsent)}>
                Retry
              </Button>
            )
          }
        />
      )}
    </MessagePrimitive.Root>
  );
}

const parts = { Text: TextPart };

function AssistantMessage() {
  const own = useAuiState(custom);
  const { agent, by, role, on, work, notice, approvals } = own;
  const actions = useActions();
  if (own.exchange)
    return (
      <Exchange
        said={own.exchange}
        text={(index) => <MessagePrimitive.PartByIndex index={index} components={parts} />}
        onOpenAgent={actions.openAgent}
      />
    );
  return (
    <MessagePrimitive.Root
      className="reply"
      data-speaker={agent ? '' : undefined}
      // A member's work reads one step quieter than the lead's words to the person.
      data-team={(role && role !== 'supervisor') || undefined}
    >
      {agent && own.head && (
        <Speaker
          name={agent}
          detail={on ? `on ${on}` : role && roleName[role]}
          time={own.time}
          // Only a team chat has a panel to open: its agents have roles.
          onOpen={by && role ? () => actions.openAgent(by) : undefined}
        />
      )}
      {work && <WorkRow work={work} onOpenTask={actions.openTask} />}
      <div className="reply-body">
        <MessagePrimitive.Parts components={parts} />
      </div>
      {approvals?.map((a) => (
        <AskApproval key={a.request} approval={a} agent={agent ?? ''} actions={actions} />
      ))}
      {notice && <ReplyNotice notice={notice} actions={actions} />}
    </MessagePrimitive.Root>
  );
}

/** A reply's text; the one a search hit opened at is wrapped, to centre and tint it alone. */
function TextPart({ text }: { text: string }) {
  return useContext(FoundContext) === text ? (
    <div className="found-part">
      <MarkdownText />
    </div>
  ) : (
    <MarkdownText />
  );
}

/** An approval: sending shows on the card, and a refused answer shows under it, to answer again. */
function AskApproval({
  approval: a,
  agent,
  actions,
}: {
  approval: Approval;
  agent: string;
  actions: Actions;
}) {
  const card = useRef<HTMLDivElement>(null);
  const failure = a.answer?.failure;
  const sending = a.answer && !failure;
  // The answer's button went while it was sent: on a refusal, focus goes back to it.
  useEffect(() => {
    if (!failure || document.activeElement !== document.body) return;
    const slot = a.answer?.allow ? 'approval-card-allow-once' : 'approval-card-deny';
    card.current?.querySelector<HTMLElement>(`[data-slot="${slot}"]`)?.focus();
  }, [failure, a.answer?.allow]);
  const { tool, command } = asked(a);
  return (
    <>
      <ApprovalCard
        ref={card}
        state={sending ? 'running' : 'request'}
        statusLabel={a.answer?.allow ? 'Allowing…' : 'Denying…'}
        title={`Allow ${tool}?`}
        command={command}
        data-approval={a.request}
        description={
          tool === 'wait check' ? 'Runs in sh (Git Bash on Windows), outside the engine sandbox.' : undefined
        }
        style={{ overflowWrap: 'anywhere' }}
        subtitle={agent}
        allowOnceLabel="Allow"
        denyLabel="Deny"
        onAllowOnce={() => actions.approve(a.request, true)}
        onDeny={() => actions.approve(a.request, false)}
      />
      {failure && <FailureNotice failure={failure} onReference={actions.reference} />}
    </>
  );
}

function ReplyNotice({ notice, actions }: { notice: TurnNotice; actions: Actions }) {
  const { turn } = notice;
  if (turn.status === 'stopped') {
    return (
      <Notice
        title="Stopped"
        action={
          <Button type="button" variant="outline" onClick={actions.carryOn}>
            Continue
          </Button>
        }
      />
    );
  }
  if (turn.held === 'claude' || turn.held === 'codex') {
    return (
      <Notice
        title={turn.why ?? (turn.held === 'codex' ? 'Codex is downloading' : 'Claude Code is downloading')}
      />
    );
  }
  if (turn.status === 'waiting_sign_in') {
    return (
      <Notice
        title="Waiting for sign-in"
        action={
          <Button type="button" variant="outline" onClick={actions.signIn}>
            Sign in
          </Button>
        }
      />
    );
  }
  return (
    <Notice
      danger
      title={notice.replied ? 'Reply failed' : 'Send failed'}
      reference={{ ref: turn.ref, open: () => actions.diagnostics(turn.id) }}
      links={<DiagnosticsLink open={() => actions.diagnostics(turn.id)} />}
      action={
        <Button type="button" variant="outline" onClick={() => actions.retry(turn.id)}>
          Retry
        </Button>
      }
    />
  );
}

export function DiagnosticsLink({ open }: { open: () => void }) {
  return (
    <button type="button" className="link" onClick={open}>
      Diagnostics
    </button>
  );
}
