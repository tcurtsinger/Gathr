import { useEffect, useState } from 'react';
import { api, type BoardLine, type Failure } from '../api';
import { listen } from '../live';
import './board.css';

const STATES: Record<BoardLine['state'], string> = {
  to_do: 'To do',
  doing: 'Doing',
  in_review: 'In review',
  done: 'Done',
};

/** The chat's board, the room's sidebar tab: read only. */
export function Board({ chat, onFailure }: { chat: string; onFailure: (f: Failure) => void }) {
  const [lines, setLines] = useState<BoardLine[] | null>(null);
  useEffect(() => {
    const load = () => api.board(chat).then((b) => setLines(b.lines), onFailure);
    load();
    return listen((event) => {
      if (event.type === 'board' && event.chat === chat) setLines(event.lines);
      if (event.type === 'lagged') load();
    });
  }, [chat, onFailure]);
  if (!lines) return null;
  return (
    <div className="board">
      {lines.length === 0 && <p className="board-empty">No work yet</p>}
      {lines.map((line) => (
        <div key={line.id} className="board-line">
          <div>{line.text}</div>
          <div className="board-meta">
            <span className={`board-state ${line.state}`}>{STATES[line.state]}</span>· {line.name}
          </div>
        </div>
      ))}
    </div>
  );
}
