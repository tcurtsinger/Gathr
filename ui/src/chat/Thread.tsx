import {
  AssistantRuntimeProvider,
  groupPartByType,
  MessagePrimitive,
  ThreadPrimitive,
  useAuiState,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { ArrowDown } from 'lucide-react';
import { createContext, type ReactNode, useContext, useMemo, useRef } from 'react';
import { ApprovalCard } from '@/components/assistant-ui/elements/approval-card';
import { SpeakerIdentity } from '@/components/assistant-ui/elements/speaker-identity';
import { ToolFallback } from '@/components/assistant-ui/elements/tool-fallback.aui';
import {
  ToolGroupContent,
  ToolGroupRoot,
  ToolGroupTrigger,
} from '@/components/assistant-ui/elements/tool-group.aui';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import type { Agent } from '../api';
import { MarkdownText } from '../components/Markdown';
import { Notice } from '../components/Notice';
import { type Custom, type SendFailure, type TurnNotice, toThread } from './messages';
import { useOlder } from './older';
import type { ChatState } from './state';
import './thread.css';

/** What a reply's notices and approvals can do. Stable for a chat, so replies don't redraw for it. */
export interface Actions {
  diagnostics: (turn?: string) => void;
  signIn: () => void;
  retry: (turn: string) => void;
  /** Sends a message again that failed before any turn began. */
  resend: (failed: SendFailure) => void;
  carryOn: () => void;
  approve: (request: string, allow: boolean) => void;
}

const ActionsContext = createContext<Actions | null>(null);
const useActions = () => useContext(ActionsContext) as Actions;

interface Props {
  state: ChatState;
  agents: Agent[];
  unsent: SendFailure[];
  running: boolean;
  actions: Actions;
  /** Shown above the messages: the chat's own load failure. */
  header?: ReactNode;
  empty: boolean;
  /** The composer, kept below the messages as they scroll. */
  children: ReactNode;
}

/** The chat's messages, drawn by assistant-ui's thread over Gathr's own chat state (0030). */
export function Thread({ state, agents, unsent, running, actions, header, empty, children }: Props) {
  const cache = useRef(new Map());
  const all = useMemo(() => toThread(state, agents, unsent, cache.current), [state, agents, unsent]);
  const viewport = useRef<HTMLDivElement>(null);
  const { messages, onScroll } = useOlder(all, viewport);
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
        <ThreadPrimitive.Root className="thread">
          <ThreadPrimitive.Viewport ref={viewport} className="thread-viewport" onScroll={onScroll}>
            <div className="thread-column">
              {header}
              {empty && <p className="empty">No messages yet</p>}
              <div className="thread-messages">
                <ThreadPrimitive.Messages>{() => <ThreadMessage />}</ThreadPrimitive.Messages>
              </div>
              <ThreadPrimitive.ViewportFooter className="thread-footer">
                <ThreadPrimitive.ScrollToBottom asChild>
                  <TooltipIconButton tooltip="Scroll to bottom" variant="outline" className="thread-bottom">
                    <ArrowDown />
                  </TooltipIconButton>
                </ThreadPrimitive.ScrollToBottom>
                {children}
              </ThreadPrimitive.ViewportFooter>
            </div>
          </ThreadPrimitive.Viewport>
        </ThreadPrimitive.Root>
      </ActionsContext>
    </AssistantRuntimeProvider>
  );
}

function ThreadMessage() {
  const role = useAuiState((s) => s.message.role);
  return role === 'user' ? <UserMessage /> : <AssistantMessage />;
}

const custom = (s: { message: { metadata: { custom: Record<string, unknown> } } }) =>
  s.message.metadata.custom as Custom;

function UserMessage() {
  const { unsent } = useAuiState(custom);
  const actions = useActions();
  return (
    <MessagePrimitive.Root className="user-message">
      <div className="user-bubble">
        <MessagePrimitive.Parts />
      </div>
      {unsent && (
        <Notice
          danger
          title={unsent.failure.error}
          reference={
            unsent.failure.ref ? { ref: unsent.failure.ref, open: () => actions.diagnostics() } : undefined
          }
          links={<DiagnosticsLink open={() => actions.diagnostics()} />}
          action={
            <button type="button" onClick={() => actions.resend(unsent)}>
              Retry
            </button>
          }
        />
      )}
    </MessagePrimitive.Root>
  );
}

const groupBy = groupPartByType({ 'tool-call': ['group-tool'] });

function AssistantMessage() {
  const { agent, notice, approvals } = useAuiState(custom);
  const actions = useActions();
  return (
    <MessagePrimitive.Root className="reply">
      {agent && (
        <SpeakerIdentity className="mb-2" turns={[{ id: 'author', kind: 'agent', name: agent, text: '' }]} />
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
                return <MarkdownText />;
              case 'tool-call':
                return <ToolFallback {...part} />;
              default:
                return null;
            }
          }}
        </MessagePrimitive.GroupedParts>
      </div>
      {approvals?.map((a) => (
        <ApprovalCard
          key={a.request}
          state="request"
          title={`Allow ${a.tool}?`}
          subtitle={agent ?? ''}
          allowOnceLabel="Allow"
          denyLabel="Deny"
          onAllowOnce={() => actions.approve(a.request, true)}
          onDeny={() => actions.approve(a.request, false)}
        />
      ))}
      {notice && <ReplyNotice notice={notice} actions={actions} />}
    </MessagePrimitive.Root>
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

function ReplyNotice({ notice, actions }: { notice: TurnNotice; actions: Actions }) {
  const { turn } = notice;
  if (turn.status === 'stopped') {
    return (
      <Notice
        title="Stopped"
        action={
          <button type="button" onClick={actions.carryOn}>
            Continue
          </button>
        }
      />
    );
  }
  if (turn.status === 'waiting_sign_in') {
    return (
      <Notice
        title="Waiting for sign-in"
        action={
          <button type="button" onClick={actions.signIn}>
            Sign in
          </button>
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
        <button type="button" onClick={() => actions.retry(turn.id)}>
          Retry
        </button>
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
