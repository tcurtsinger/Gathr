import { StateWord } from '../components/StateWord';
import type { MainState } from './messages';
import { useJump } from './Thread';

/**
 * A main message's state at its name line's right (0075), from the same function the Tasks list reads: its PR first,
 * then the mark's shape and its word. An ask's Done leads to its newest result.
 */
export function MainWord({ state }: { state?: MainState }) {
  const jump = useJump();
  if (!state) return null;
  const word = <StateWord said={state.said} className="main-state-word" />;
  const { link } = state;
  return (
    <span className="main-state">
      {state.pr != null && <span className="main-state-pr">PR #{state.pr}</span>}
      {link ? (
        <button type="button" className="main-state-link" onClick={() => jump(link)}>
          {word}
        </button>
      ) : (
        word
      )}
    </span>
  );
}
