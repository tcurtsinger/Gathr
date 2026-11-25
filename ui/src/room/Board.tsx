import { ChevronRight } from 'lucide-react';
import { useEffect, useState } from 'react';
import { api, type BoardLine } from '../api';
import { Face } from '../components/Face';
import { listen } from '../live';
import './board.css';

const STATES: Record<BoardLine['state'], string> = {
  to_do: 'To do',
  doing: 'Doing',
  in_review: 'In review',
  done: 'Done',
};

/**
 * The chat's board, on the panel's Home, each line with its owner's face: read only. A read that fails says nothing:
 * the live socket reloads it.
 */
export function Board({ chat }: { chat: string }) {
  const [lines, setLines] = useState<BoardLine[] | null>(null);
  const [done, setDone] = useState(false);
  useEffect(() => {
    setDone(false);
    const load = () =>
      api.board(chat).then(
        (b) => setLines(b.lines),
        () => {},
      );
    load();
    return listen((event) => {
      if (event.type === 'board' && event.chat === chat) setLines(event.lines);
      if (event.type === 'lagged') load();
    });
  }, [chat]);
  if (!lines) return null;
  return (
    <div className="board">
      {lines.length === 0 && <p className="board-empty">No work yet</p>}
      {Object.entries(STATES).map(([state, label]) => {
        const group = lines.filter((line) => line.state === state);
        if (!group.length) return null;
        return (
          <section key={state}>
            <h2>
              {state === 'done' ? (
                <button type="button" aria-expanded={done} onClick={() => setDone(!done)}>
                  <ChevronRight aria-hidden="true" />
                  {label} ({group.length})
                </button>
              ) : (
                `${label} (${group.length})`
              )}
            </h2>
            {(state !== 'done' || done) &&
              group.map((line) => (
                <div key={line.id} className="board-line">
                  <Face agent={{ id: line.owner, name: line.name }} size={20} />
                  <div>
                    <div>{line.text}</div>
                    <div className="board-meta">
                      <span className={`board-state ${line.state}`}>{STATES[line.state]}</span>
                    </div>
                  </div>
                </div>
              ))}
          </section>
        );
      })}
    </div>
  );
}
