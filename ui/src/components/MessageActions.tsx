import { ThumbsUp } from 'lucide-react';
import { IconButton } from './IconButton';
import './message-actions.css';

/**
 * The small row on a message's bubble, at its top right corner (DESIGN.md: MessageActions): shown while the message
 * is pointed at or holds keyboard focus. It holds the person's like (0057); Reply joins it later.
 */
export function MessageActions({ like }: { like: { given: boolean; onToggle: () => void } }) {
  return (
    <div className="message-actions">
      <IconButton aria-label="Like" aria-pressed={like.given} onClick={like.onToggle}>
        <ThumbsUp aria-hidden="true" />
      </IconButton>
    </div>
  );
}
