import type { Mark } from './marks';

/**
 * Each mark on a 12 box (0071): a disc in the state's colour with a shape cut out in the colour of the surface,
 * except Working, a three-quarter ring, and Needs you and Free, plain dots. At 20 and under a part that would blur
 * is drawn bolder; at 24 and under Stopped is a plain square, as a square cut from a disc reads as a ring there.
 */
const CUT: Partial<Record<Mark, { big: string; small: string }>> = {
  cant: {
    big: 'M5.15 3.25a.85.85 0 0 1 1.7 0v3a.85.85 0 0 1-1.7 0ZM7 9.15a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z',
    small: 'M5 3.2a1 1 0 0 1 2 0v3a1 1 0 0 1-2 0ZM7.1 9.3a1.1 1.1 0 1 1-2.2 0 1.1 1.1 0 0 1 2.2 0Z',
  },
  stopped: {
    big: 'M4.55 3.75h2.9a.8.8 0 0 1 .8.8v2.9a.8.8 0 0 1-.8.8h-2.9a.8.8 0 0 1-.8-.8v-2.9a.8.8 0 0 1 .8-.8Z',
    small: '',
  },
  blocked: {
    big: 'M3.5 5.1h5a.9.9 0 0 1 0 1.8h-5a.9.9 0 0 1 0-1.8Z',
    small: 'M3.4 4.9h5.2a1.1 1.1 0 0 1 0 2.2H3.4a1.1 1.1 0 0 1 0-2.2Z',
  },
};
const HAND = { big: 'M6 3.1V6l2.1 1.3', small: 'M6 2.9V6.1h2.6' };

/** The mark alone: on a chat's row or a card's head, at `size`. */
export function StateMark({ state, size, spin }: { state: Mark; size: number; spin?: boolean }) {
  const small = size <= 20;
  return (
    <i className="mark" data-state={state} data-spin={spin || undefined} aria-hidden="true">
      <svg viewBox="0 0 12 12" aria-hidden="true">
        {shape(state, small, size <= 24)}
      </svg>
    </i>
  );
}

function shape(state: Mark, small: boolean, square: boolean) {
  if (state === 'working') return <path className="mark-arc" d="M6 1.5a4.5 4.5 0 1 1-4.5 4.5" />;
  if (state === 'stopped' && square)
    return <rect className="mark-disc" x="1" y="1" width="10" height="10" rx="2" />;
  const cut = CUT[state];
  return (
    <>
      <circle className="mark-disc" cx="6" cy="6" r="6" />
      {cut && <path className="mark-cut" d={small && cut.small ? cut.small : cut.big} />}
      {state === 'waiting' && (
        <path className="mark-hand" data-small={small || undefined} d={small ? HAND.small : HAND.big} />
      )}
    </>
  );
}
