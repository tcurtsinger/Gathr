import type { Mark } from './marks';

/**
 * Each mark on a 12 box (0071): Needs you a plain dot, Can't run a disc with ! cut out in the colour of the surface
 * (bolder at 20 and under, where the thin cut would blur), Working a three-quarter ring.
 */
const CANT = {
  big: 'M5.15 3.25a.85.85 0 0 1 1.7 0v3a.85.85 0 0 1-1.7 0ZM7 9.15a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z',
  small: 'M5 3.2a1 1 0 0 1 2 0v3a1 1 0 0 1-2 0ZM7.1 9.3a1.1 1.1 0 1 1-2.2 0 1.1 1.1 0 0 1 2.2 0Z',
};

/** The mark alone: on a chat's row or a card's head, at `size`. */
export function StateMark({ state, size, spin }: { state: Mark; size: number; spin?: boolean }) {
  return (
    <i className="mark" data-state={state} data-spin={spin || undefined} aria-hidden="true">
      <svg viewBox="0 0 12 12" aria-hidden="true">
        {state === 'working' ? (
          <path className="mark-arc" d="M6 1.5a4.5 4.5 0 1 1-4.5 4.5" />
        ) : (
          <>
            <circle className="mark-disc" cx="6" cy="6" r="6" />
            {state === 'cant' && <path className="mark-cut" d={size <= 20 ? CANT.small : CANT.big} />}
          </>
        )}
      </svg>
    </i>
  );
}
