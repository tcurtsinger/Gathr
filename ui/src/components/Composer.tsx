import type { KeyboardEvent, ReactNode, Ref } from 'react';
import './composer.css';

interface Props {
  draft: string;
  onDraft: (text: string) => void;
  onSend: () => void;
  /** Set while a turn runs: the button stops it instead of sending. */
  onStop?: () => void;
  inputRef?: Ref<HTMLTextAreaElement>;
  /** Controls at the bottom left, such as the agent picker. */
  tools?: ReactNode;
}

/** The one composer. Enter sends, Shift+Enter is a new line. */
export function Composer({ draft, onDraft, onSend, onStop, inputRef, tools }: Props) {
  const canSend = draft.trim().length > 0;
  const keyDown = (e: KeyboardEvent) => {
    if (e.key !== 'Enter' || e.shiftKey || e.nativeEvent.isComposing) return;
    e.preventDefault();
    if (canSend) onSend();
  };
  return (
    <div className="composer">
      <textarea
        ref={inputRef}
        aria-label="Message"
        placeholder="Message"
        value={draft}
        onChange={(e) => onDraft(e.target.value)}
        onKeyDown={keyDown}
      />
      <div className="composer-bottom">
        {tools}
        {onStop && !canSend ? (
          <button type="button" className="round" aria-label="Stop" onClick={onStop}>
            <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
              <rect width="12" height="12" rx="2" fill="currentColor" />
            </svg>
          </button>
        ) : (
          <button type="button" className="round" aria-label="Send" disabled={!canSend} onClick={onSend}>
            <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
              <path d="M8 13V3M3.5 7.5 8 3l4.5 4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}
