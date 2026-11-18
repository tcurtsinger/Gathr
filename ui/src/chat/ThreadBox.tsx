import { MessagePrimitive } from '@assistant-ui/react';
import { createContext, type Ref, useContext, useEffect, useRef } from 'react';
import { Composer, type Person } from '../components/Composer';
import type { Picked } from '../components/mention';
import { Refused } from '../tasks/Question';
import type { Answer } from './messages';
import { useActions } from './Thread';

/** The box open for a reply, as the chat keeps it: its thread's own draft, and who gets it. */
export interface Box {
  topic: string;
  draft: string;
  picks: Picked[];
  onDraft: (text: string, picks: Picked[]) => void;
  onSend: () => void;
  onClose: () => void;
  people: readonly Person[];
  to?: { id: string; name: string; joins?: boolean };
  hint: string;
  /** A Discuss the service refused: why, and its reference. */
  refused?: { why: string; ref: string | null };
}

/** The box open for a reply, read only by the box. */
export const BoxContext = createContext<Box | undefined>(undefined);

/** Reply on a message, or none where it offers none: its agent was removed, or the chat is archived. */
export function useReply(answer: Answer | undefined) {
  const actions = useActions();
  if (!answer || !actions.canReply?.(answer)) return undefined;
  return (opener: HTMLElement) => actions.reply?.(answer, opener);
}

/**
 * The one composer, placed at the foot of the thread it is open for (`#replybox` A): one row with who gets it by Send
 * and a cross. Focus goes into it as it opens; Escape or the cross closes it, and focus goes back to the Reply that
 * opened it.
 */
export function ThreadBox({ topic }: { topic: string }) {
  const box = useContext(BoxContext);
  const actions = useActions();
  const input = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    input.current?.focus({ preventScroll: false });
    input.current?.scrollIntoView({ block: 'nearest' });
  }, []);
  if (!box || box.topic !== topic) return null;
  return (
    <MessagePrimitive.Root className="thread-box" data-thread-item="">
      <Composer
        draft={box.draft}
        picks={box.picks}
        onDraft={box.onDraft}
        onSend={box.onSend}
        onClose={box.onClose}
        inputRef={input as Ref<HTMLTextAreaElement>}
        label="Reply"
        hint={box.hint}
        people={box.people}
        to={box.to}
      />
      {box.refused && <Refused word="Not sent" refusal={box.refused} onReference={actions.reference} />}
    </MessagePrimitive.Root>
  );
}
