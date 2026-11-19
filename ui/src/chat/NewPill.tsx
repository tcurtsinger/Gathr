import { ArrowDown, ArrowUp } from 'lucide-react';
import { useContext, useSyncExternalStore } from 'react';
import { type Fresh, ReadsContext } from './reads';

const noop = () => () => {};
const NONE = { count: 0 } as Fresh;

/**
 * Where the person left off, at the foot (0077, the owner's call of 2026-11-18): "1 New", "2 New", centred just above the
 * message box, over the messages. It counts every block of someone else's not seen yet, wherever it is; a press goes
 * to the oldest. Only it draws again as the count changes, never the chat.
 */
export function NewPill({ onGo }: { onGo: (fresh: Fresh) => void }) {
  const reads = useContext(ReadsContext);
  const fresh = useSyncExternalStore(reads?.subscribe ?? noop, () => reads?.fresh() ?? NONE);
  if (!fresh.count) return null;
  const Arrow = fresh.up ? ArrowUp : ArrowDown;
  const label =
    fresh.count === 1 ? '1 new message. Go to it' : `${fresh.count} new messages. Go to the oldest`;
  return (
    <button type="button" className="new-pill" aria-label={label} onClick={() => onGo(fresh)}>
      <Arrow aria-hidden="true" />
      {fresh.count > 99 ? '99+' : fresh.count} New
    </button>
  );
}
