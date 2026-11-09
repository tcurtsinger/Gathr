import type { KeyboardEvent, ReactNode, Ref } from 'react';
import {
  ComposerActions,
  ComposerBar,
  ComposerSend,
  ComposerToolbar,
  Composer as Frame,
} from '@/components/assistant-ui/elements/composer';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import './composer.css';

interface Props {
  draft: string;
  onDraft: (text: string) => void;
  onSend: () => void;
  /** Set while a turn runs: the button stops it instead of sending. */
  onStop?: () => void;
  inputRef?: Ref<HTMLTextAreaElement>;
  /** Controls at the bottom right, before Send, such as the agent picker. */
  tools?: ReactNode;
}

/** The one composer, on the registry's composer parts. Enter sends, Shift+Enter is a new line. */
export function Composer({ draft, onDraft, onSend, onStop, inputRef, tools }: Props) {
  const canSend = draft.trim().length > 0;
  const stopping = !!onStop && !canSend;
  // Stop names its scope (the prototype): it ends the chat's turns and the commands they started.
  const label = stopping ? 'Stop this chat now' : 'Send';
  const keyDown = (e: KeyboardEvent) => {
    if (e.key !== 'Enter' || e.shiftKey || e.nativeEvent.isComposing) return;
    e.preventDefault();
    if (canSend) onSend();
  };
  return (
    <Frame className="composer">
      <ComposerBar>
        <textarea
          ref={inputRef}
          className="composer-input"
          aria-label="Message"
          placeholder="Message"
          rows={1}
          value={draft}
          onChange={(e) => onDraft(e.target.value)}
          onKeyDown={keyDown}
        />
        <ComposerToolbar>
          <ComposerActions />
          <ComposerActions>
            {tools}
            <TooltipProvider delayDuration={0}>
              <Tooltip>
                <TooltipTrigger asChild>
                  <ComposerSend
                    aria-label={label}
                    streaming={stopping}
                    idle={!canSend}
                    disabled={!canSend && !stopping}
                    onClick={stopping ? onStop : onSend}
                  />
                </TooltipTrigger>
                <TooltipContent>{label}</TooltipContent>
              </Tooltip>
            </TooltipProvider>
          </ComposerActions>
        </ComposerToolbar>
      </ComposerBar>
    </Frame>
  );
}
