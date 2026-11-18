import { Reply, ThumbsUp } from 'lucide-react';
import { IconButton } from './IconButton';
import './message-actions.css';

/**
 * The small row on a message's bubble, at its top right corner (DESIGN.md: MessageActions): shown while the message
 * is pointed at or holds keyboard focus. Reply first (0076), then the person's like (0057); a message offers what its
 * kind allows (`chat/answer.ts`, `chat/likes.ts`), and a row with neither isn't drawn.
 */
export function MessageActions({
  like,
  reply,
}: {
  like?: { given: boolean; onToggle: () => void };
  /** Opens the box for a reply to it; the button it was pressed on gets focus back when the box closes. */
  reply?: (opener: HTMLElement) => void;
}) {
  if (!like && !reply) return null;
  return (
    <div className="message-actions">
      {reply && (
        <IconButton aria-label="Reply" onClick={(e) => reply(e.currentTarget)}>
          <Reply aria-hidden="true" />
        </IconButton>
      )}
      {like && (
        <IconButton aria-label="Like" aria-pressed={like.given} onClick={like.onToggle}>
          <ThumbsUp aria-hidden="true" />
        </IconButton>
      )}
    </div>
  );
}
