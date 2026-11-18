import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  ThreadPrimitive,
  useAuiState,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { ArrowDown } from 'lucide-react';
import { createContext, type ReactNode, useCallback, useContext, useMemo, useRef } from 'react';
import { IconButton } from '../components/IconButton';
import { type Questions, QuestionsContext } from './Asked';
import { ChatMap, useTicks } from './ChatMap';
import { useEdges } from './edges';
import { useFound, useLatest } from './found';
import { GathrMessage } from './Gathr';
import type { SendFailure } from './messages';
import { useOlder } from './older';
import { AssistantMessage, UserMessage } from './Replies';
import { FoundContext } from './TurnParts';
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
  /** Puts Continue in the message box, to the agent whose turn stopped. */
  carryOn: (agent?: string) => void;
  approve: (request: string, allow: boolean) => void;
  openTask: (task: number) => void;
  /** Opens an agent's panel in the workspace. */
  openAgent: (agent: string) => void;
  /** Opens a plan's tab in the workspace at a version (0062). */
  openPlan?: (plan: number, version: number) => void;
  /** Gives the person's like on a message, answering the waiting questions it holds, or takes it back (0057). */
  like?: (message: string, questions: number[], give: boolean) => void;
}

const ActionsContext = createContext<Actions | null>(null);
export const useActions = () => useContext(ActionsContext) as Actions;

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
  /** What its questions are answered with, in place (0073). */
  questions?: Questions;
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
  /** Opened from the conversation map: it starts at the top of the view, and focus stays on its tick. */
  top?: boolean;
  at: number;
}

// One function for the life of the page: the runtime keeps what it converted only while its converter is
// the same one, and would convert every message again on each streamed token otherwise.
const asIs = (m: ThreadMessageLike) => m;

/** The chat's timeline, drawn by assistant-ui's thread over Gathr's own chat state (0030, 0037). */
export function Thread(props: Props) {
  const { running, actions, header, after, empty, children } = props;
  const viewport = useRef<HTMLDivElement>(null);
  const { messages, onScroll, reach } = useOlder(props.messages, viewport);
  // A tick of the map opens its message as a search hit opens: whichever was picked last.
  const { target, own, pick } = useLatest(props.target);
  const onJump = useCallback((id: string) => pick({ id, top: true, at: performance.now() }), [pick]);
  const found = useFound(props.messages, viewport, reach, target, own ? undefined : props.onTarget);
  useEdges(viewport);
  const runtime = useExternalStoreRuntime({
    messages,
    convertMessage: asIs,
    isRunning: running,
    // The composer is Gathr's own and sends through useChat, never through the runtime.
    onNew: async () => {},
  });
  const stable = useStable(actions);
  const ticks = useTicks(props.messages);
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <ActionsContext value={stable}>
        <QuestionsContext value={props.questions ?? null}>
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
                      <IconButton
                        aria-label="Scroll to bottom"
                        variant="outline"
                        className="thread-bottom bg-background"
                      >
                        <ArrowDown />
                      </IconButton>
                    </ThreadPrimitive.ScrollToBottom>
                    {children}
                  </ThreadPrimitive.ViewportFooter>
                </div>
              </ThreadPrimitive.Viewport>
              <ChatMap ticks={ticks} viewport={viewport} onJump={onJump} />
            </ThreadPrimitive.Root>
          </FoundContext>
        </QuestionsContext>
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
      carryOn: (agent) => latest.current.carryOn(agent),
      approve: (request, allow) => latest.current.approve(request, allow),
      openTask: (task) => latest.current.openTask(task),
      openAgent: (agent) => latest.current.openAgent(agent),
      openPlan: (plan, version) => latest.current.openPlan?.(plan, version),
      like: (message, questions, give) => latest.current.like?.(message, questions, give),
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

/** The person's words, what Gathr says (0073), or an agent's. */
function ThreadMessage() {
  const role = useAuiState((s) => s.message.role);
  return role === 'user' ? <UserMessage /> : role === 'system' ? <GathrMessage /> : <AssistantMessage />;
}
