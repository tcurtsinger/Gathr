import { Clock } from 'lucide-react';

/**
 * Scheduled's mark, on the rail, on a schedule's page and on its lines in the chat: a stand-in, lucide's Clock, until
 * the owner gives theirs. Only this file changes then.
 */
export function ScheduledMark() {
  return <Clock aria-hidden="true" />;
}
