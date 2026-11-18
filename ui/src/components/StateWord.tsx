import { Check } from 'lucide-react';
import type { Said } from '../tasks/rows';
import { WORD } from './marks';
import { Outcome } from './Outcome';
import { StateMark } from './StateMark';

/**
 * A task's or a main message's state, one way everywhere (0071's shapes): the mark's shape, still, and its word; an
 * ending's icon; Needs you in the attention blue; Done with a tick.
 */
export function StateWord({ said, className }: { said: Said; className: string }) {
  if ('done' in said)
    return (
      <span className={className} data-state="done">
        <Check aria-hidden="true" />
        {said.word ?? 'Done'}
      </span>
    );
  if ('you' in said)
    return (
      <span className={className} data-state="you">
        <StateMark state="needs" size={12} />
        {said.word ?? 'Waiting'}
      </span>
    );
  if ('word' in said) return <span className={className}>{said.word}</span>;
  if ('mark' in said)
    return (
      <span className={className} data-state={said.mark}>
        <StateMark state={said.mark} size={12} />
        {said.on ? `${WORD[said.mark]} on ${said.on}` : WORD[said.mark]}
      </span>
    );
  return <Outcome out={said.icon} className={className} />;
}
