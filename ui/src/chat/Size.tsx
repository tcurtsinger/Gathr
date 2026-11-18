import { ChevronDown } from 'lucide-react';
import { type FocusEvent, type KeyboardEvent, useEffect, useRef, useState } from 'react';
import { OptionList } from '@/components/assistant-ui/elements/option-list';
import { api, type Failure } from '../api';
import { Face } from '../components/Face';
import type { AskSize } from './asks';
import { useMarks } from './faceMarks';
import './size.css';

const SIZES = [
  { id: 'repair', label: 'Just fix it.' },
  { id: 'new', label: 'Plan it with me first.' },
];

const STEP: Record<string, number> = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };

/**
 * An ask's size at the end of its message's name line (0061): who sized it, when an agent did, and the word. It
 * opens in place to the two sizes, the pressed one ticked, the arrow keys moving between them. Choosing one closes
 * them, and choosing the other asks the service: the word redraws from the live update, and a refusal goes to
 * `onFailure`. An archived chat shows the word alone.
 */
export function SizeWord({ size, onFailure }: { size: AskSize; onFailure: (failure?: Failure) => void }) {
  const archived = useMarks()?.archived;
  const [open, setOpen] = useState(false);
  const word = useRef<HTMLButtonElement>(null);
  const choices = useRef<HTMLSpanElement>(null);
  // Focus goes back to the word when the choices close by the keyboard or a choice.
  const back = useRef(false);
  useEffect(() => {
    if (open) choices.current?.querySelector<HTMLElement>('[aria-pressed="true"]')?.focus();
    else if (back.current) word.current?.focus();
    back.current = false;
  }, [open]);
  const said = size.size === 'repair' ? 'Fix' : 'Plan';
  const face = size.by && <Face agent={{ id: size.by.agent, name: size.by.name }} size={16} />;
  if (archived)
    return (
      <span className="ask-size">
        {face}
        <span>{said}</span>
      </span>
    );
  const close = () => {
    back.current = true;
    setOpen(false);
  };
  const choose = (next: string) => {
    close();
    if (next === size.size) return;
    onFailure(undefined);
    api.resizeAsk(size.ask, next as AskSize['size']).catch(onFailure);
  };
  const keys = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      return close();
    }
    const buttons = [...e.currentTarget.querySelectorAll('button')];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (STEP[e.key] === undefined || at < 0) return;
    e.preventDefault();
    buttons[(at + STEP[e.key] + buttons.length) % buttons.length].focus();
  };
  // Focus leaving the choices for anywhere else closes them.
  const left = (e: FocusEvent) => {
    if (!e.currentTarget.contains(e.relatedTarget)) setOpen(false);
  };
  if (open)
    return (
      <span ref={choices} className="ask-size-choices">
        <OptionList
          presentation="inline"
          aria-label="Size"
          onKeyDown={keys}
          onBlur={left}
          options={SIZES}
          value={[size.size]}
          onValueChange={([next]) => choose(next)}
        />
      </span>
    );
  const by = size.by ? `, by ${size.by.name}` : size.yours ? ', by you' : '';
  return (
    <button
      ref={word}
      type="button"
      className="ask-size"
      aria-label={`Size: ${said}${by}`}
      onClick={() => setOpen(true)}
    >
      {face}
      <span>{said}</span>
      <ChevronDown aria-hidden="true" />
    </button>
  );
}
