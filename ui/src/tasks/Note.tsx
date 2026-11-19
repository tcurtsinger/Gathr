import type { ReviewNote } from '../api';
import './tasks.css';

const KINDS: Record<ReviewNote['kind'], string> = {
  wrong: 'Wrong',
  unasked: 'Not asked for',
  suggestion: 'Suggestion',
};

/**
 * A review's or a plan check's note in a line (0053): "Not asked for · src/cache.rs: what it is", its file in
 * monospace. The Tasks view, the plan's tab and the findings chip all draw it so.
 */
export function Note({ note }: { note: ReviewNote }) {
  return (
    <span className="note">
      <span className="note-kind" data-kind={note.kind}>
        {KINDS[note.kind]}
      </span>
      {' · '}
      {note.file && (
        <>
          <code className="note-file">{note.file}</code>
          {': '}
        </>
      )}
      {note.says}
    </span>
  );
}
