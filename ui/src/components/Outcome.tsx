import { Ban, CircleAlert, GitMerge } from 'lucide-react';
import './outcome.css';

const ICONS = { merge: GitMerge, failed: CircleAlert, cancelled: Ban };
const WORDS = { merge: 'Merging', failed: 'Failed', cancelled: 'Cancelled' };

export type Out = keyof typeof ICONS;

/** How a task ended or stands, as Gathr's icon and its word, Failed in the danger colour: a task's row and the chat. */
export function Outcome({ out, className }: { out: Out; className?: string }) {
  const Icon = ICONS[out];
  return (
    <span className={className ? `outcome ${className}` : 'outcome'} data-out={out}>
      <Icon aria-hidden="true" />
      {WORDS[out]}
    </span>
  );
}
