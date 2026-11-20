import { useEffect, useState } from 'react';
import { api, type BoardLine, type Failure } from './api';
import { listen } from './live';
import './board.css';

const STATES: Record<BoardLine['state'], string> = {
  to_do: 'To do',
  doing: 'Doing',
  in_review: 'In review',
  done: 'Done',
};

/** The chat's board, on the right of the chat: read only, and only in a chat of more than one agent. */
export function Board({ chat, onFailure }: { chat: string; onFailure: (f: Failure) => void }) {
  const [board, setBoard] = useState<{ lines: BoardLine[]; agents: number } | null>(null);
  useEffect(() => {
    const load = () => api.board(chat).then(setBoard, onFailure);
    load();
    return listen((event) => {
      if (event.type === 'board' && event.chat === chat) setBoard((b) => b && { ...b, lines: event.lines });
      if ((event.type === 'members' && event.chat === chat) || event.type === 'lagged') load();
    });
  }, [chat, onFailure]);
  if (!board || board.agents < 2) return null;
  return (
    <aside className="board" aria-label="Board">
      <h2>Board</h2>
      {board.lines.length === 0 && <p className="board-empty">No work yet</p>}
      {board.lines.map((line) => (
        <div key={line.id} className="board-line">
          <div>{line.text}</div>
          <div className="board-meta">
            <span className={`board-state ${line.state}`}>{STATES[line.state]}</span>· {line.name}
          </div>
        </div>
      ))}
    </aside>
  );
}
