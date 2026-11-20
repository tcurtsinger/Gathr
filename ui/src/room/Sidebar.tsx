import { SquareKanban } from 'lucide-react';
import type { Failure } from '../api';
import { ResizeHandle } from '../components/ResizeHandle';
import { widest } from '../components/workspace';
import { Board } from './Board';

/** The room's sidebar beside the chat, as the old chat's: its one tab, Board, is always there. */
export function RoomSidebar({ chat, onFailure }: { chat: string; onFailure: (f: Failure) => void }) {
  return (
    <aside className="workspace" aria-label="Chat sidebar">
      <ResizeHandle label="Resize chat sidebar" side="left" name="--workspace-width" min={360} max={widest} />
      <div className="workspace-tabs">
        <div className="workspace-tablist" role="tablist" aria-label="Sidebar tabs">
          <div className="workspace-tab" data-selected="true">
            <button
              type="button"
              role="tab"
              id="room-tab-board"
              aria-selected
              aria-controls="room-view-board"
            >
              <SquareKanban aria-hidden="true" />
              <span>Board</span>
            </button>
          </div>
        </div>
      </div>
      <section
        id="room-view-board"
        role="tabpanel"
        aria-labelledby="room-tab-board"
        className="workspace-view"
      >
        <Board chat={chat} onFailure={onFailure} />
      </section>
    </aside>
  );
}
