import {
  AssistantRuntimeProvider,
  groupPartByType,
  MessagePrimitive,
  type ThreadMessageLike,
  ThreadPrimitive,
  useAuiState,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { ArrowDown } from 'lucide-react';
import { createContext, type ReactNode, useContext, useEffect, useMemo, useRef } from 'react';
import { ApprovalCard } from '@/components/assistant-ui/elements/approval-card';
import { SpeakerIdentity } from '@/components/assistant-ui/elements/speaker-identity';
import { ToolFallback } from '@/components/assistant-ui/elements/tool-fallback.aui';
import {
  ToolGroupContent,
  ToolGroupRoot,
  ToolGroupTrigger,
} from '@/components/assistant-ui/elements/tool-group.aui';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import { Button } from '@/components/ui/button';
import { MarkdownText } from '../components/Markdown';
import { FailureNotice, Notice } from '../components/Notice';
import { useEdges } from './edges';
import { useFound } from './found';
import type { Custom, SendFailure, TurnNotice } from './messages';
import { useOlder } from './older';
import type { Approval } from './state';
import { GathrLine } from './TeamParts';
import { clock, roleName } from './team';
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
}

const ActionsContext = createContext<Actions | null>(null);
const useActions = () => useContext(ActionsContext) as Actions;
/** Team replies that came in while the team column was hidden: a dot by their time. */
const FreshContext = createContext<ReadonlySet<string>>(new Set());

interface Props {
  /** Its messages, from toThread. */
  messages: ThreadMessageLike[];
  running: boolean;
  actions: Actions;
  /** Its name, over the top of the column, in the split chat. */
  label?: string;
  fresh?: ReadonlySet<string>;
  /** Shown above the messages: the chat's own load failure. */
  header?: ReactNode;
  /** Shown after the latest message: the quiet "You paused" line, or who is working. */
  after?: ReactNode;
  empty: boolean;
  /** The composer, kept below the messages as they scroll. */
  children?: ReactNode;
  /** A search hit to open at: centred and tinted, the view held there until the person is back at the bottom. */
  target?: Target;
  /** The target is drawn and centred. */
  onTarget?: () => void;
}

/**
 * A thread message to open at, or with no `id` the latest one; `text` is the hit's own part of a reply, the
 * `nth` with that text. `at` is when it was picked.
 */
export interface Target {
  id?: string;
  text?: string;
  nth?: number;
  at: number;
}

const none: ReadonlySet<string> = new Set();
/** The reply part a search hit opened at. */
const FoundContext = createContext<string | undefined>(undefined);

/** A column of the chat, drawn by assistant-ui's thread over Gathr's own chat state (0030). */
export function Thread(props: Props) {
  const { running, actions, label, header, after, empty, children, target } = props;
  const viewport = useRef<HTMLDivElement>(null);
  const { messages, onScroll, reach } = useOlder(props.messages, viewport);
  const found = useFound(props.messages, viewport, reach, target, props.onTarget);
  useEdges(viewport);
  const runtime = useExternalStoreRuntime({
    messages,
    convertMessage: (m) => m,
    isRunning: running,
    // The composer is Gathr's own and sends through useChat, never through the runtime.
    onNew: async () => {},
  });
  // Every message reads the actions, so they keep one identity and call the latest ones.
  const latest = useRef(actions);
  latest.current = actions;
  const stable = useMemo<Actions>(
    () => ({
      diagnostics: (turn) => latest.current.diagnostics(turn),
      reference: (ref) => latest.current.reference(ref),
      signIn: () => latest.current.signIn(),
      retry: (turn) => latest.current.retry(turn),
      resend: (failed) => latest.current.resend(failed),
      carryOn: () => latest.current.carryOn(),
      approve: (request, allow) => latest.current.approve(request, allow),
    }),
    [],
  );
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <ActionsContext value={stable}>
        <FreshContext value={props.fresh ?? none}>
          <FoundContext value={found.text}>
            <ThreadPrimitive.Root className="thread" role={label ? 'region' : undefined} aria-label={label}>
              <ThreadPrimitive.Viewport
                ref={viewport}
                className="thread-viewport"
                scrollToBottomOnRunStart={!found.held}
                onScroll={() => {
                  onScroll();
                  found.onScroll();
                }}
              >
                {label && (
                  <div className="column-label" aria-hidden="true">
                    {label}
                  </div>
                )}
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
        </FreshContext>
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

/** What Gathr told an agent. */
function SystemItem() {
  return <GathrLine custom={useAuiState(custom)} />;
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

const groupBy = groupPartByType({ 'tool-call': ['group-tool'] });

function AssistantMessage() {
  const own = useAuiState(custom);
  const { agent, role, to, time, notice, approvals } = own;
  const actions = useActions();
  const fresh = useContext(FreshContext).has(useAuiState((s) => s.message.id));
  return (
    <MessagePrimitive.Root className="reply">
      {agent && (
        <header className="speaker">
          <SpeakerIdentity
            className="w-auto"
            turns={[
              {
                id: 'author',
                kind: 'agent',
                name: agent,
                detail: role ? roleName[role] : undefined,
                text: '',
              },
            ]}
          />
          {to && <span className="reply-to">To {to}</span>}
          {fresh && <i className="attention" role="img" aria-label="New" />}
          {time && <time dateTime={time}>{clock(time)}</time>}
        </header>
      )}
      <div className="reply-body">
        <MessagePrimitive.GroupedParts groupBy={groupBy}>
          {({ part, children }) => {
            switch (part.type) {
              case 'group-tool':
                return (
                  <ToolGroup count={part.indices.length} running={part.status.type === 'running'}>
                    {children}
                  </ToolGroup>
                );
              case 'text':
                return <TextPart text={part.text} />;
              case 'tool-call':
                return <ToolFallback {...part} />;
              default:
                return null;
            }
          }}
        </MessagePrimitive.GroupedParts>
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

function ToolGroup({ count, running, children }: { count: number; running: boolean; children: ReactNode }) {
  return (
    <ToolGroupRoot variant="ghost">
      <ToolGroupTrigger count={count} active={running} />
      <ToolGroupContent>{children}</ToolGroupContent>
    </ToolGroupRoot>
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
  const input = a.input as { command?: unknown; check?: unknown } | null | undefined;
  // Claude's hook asks for a wait check under the tool's name, with the command as `check`.
  const claudeCheck = a.tool === 'mcp__gathr__wait' ? input?.check : undefined;
  const tool = typeof claudeCheck === 'string' ? 'wait check' : a.tool;
  const command = tool === a.tool ? input?.command : claudeCheck;
  return (
    <>
      <ApprovalCard
        ref={card}
        state={sending ? 'running' : 'request'}
        statusLabel={a.answer?.allow ? 'Allowing…' : 'Denying…'}
        title={`Allow ${tool}?`}
        command={typeof command === 'string' ? command : undefined}
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
