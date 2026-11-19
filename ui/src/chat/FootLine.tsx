import { ArrowUp } from 'lucide-react';
import { useContext, useSyncExternalStore } from 'react';
import { Face } from '../components/Face';
import type { Custom } from './messages';
import { ReadsContext } from './reads';
import { clock } from './team';
import './foot-line.css';

const noop = () => () => {};

/**
 * The line at the chat's foot, above the status line (0077): the newest reply the person hasn't seen whose place isn't
 * in view, by who said it, its first words and when. A press goes there, opening its thread. None while nothing is
 * unseen; it says nothing the status line says.
 */
export function FootLine({ onGo }: { onGo: (id: string, topic: string, time: string) => void }) {
  const reads = useContext(ReadsContext);
  const id = useSyncExternalStore(reads?.subscribe ?? noop, () => reads?.foot() ?? '');
  const item = id ? reads?.item(id) : undefined;
  if (!item) return null;
  const own = (item.metadata?.custom ?? {}) as Custom;
  const parts = (item.content ?? []) as readonly { text?: string }[];
  const words = parts
    .map((p) => p.text ?? '')
    .join(' ')
    .replace(/[*_`#>]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
  // A message between agents (0072) names its writer in what it says.
  const name = own.agent ?? own.said?.from ?? 'An agent';
  const agent = own.by ?? own.said?.agent ?? '';
  return (
    <button type="button" className="foot-line" onClick={() => onGo(id, own.topic ?? '', own.time ?? '')}>
      <Face agent={{ id: agent, name }} size={20} />
      <span className="foot-line-words">
        <b>{name}</b> replied: {words}
      </span>
      <time className="foot-line-time" dateTime={own.time}>
        {clock(own.time)}
      </time>
      <ArrowUp className="foot-line-go" aria-hidden="true" />
    </button>
  );
}
