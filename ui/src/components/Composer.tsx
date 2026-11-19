import { X } from 'lucide-react';
import {
  type KeyboardEvent,
  type Ref,
  type SyntheticEvent,
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import {
  ComposerActions,
  ComposerBar,
  ComposerSend,
  ComposerToolbar,
  Composer as Frame,
} from '@/components/assistant-ui/elements/composer';
import { Face } from './Face';
import { IconButton } from './IconButton';
import { MentionList, type Person } from './MentionList';
import { follow, insert, type Picked, typing } from './mention';
import { NameLayer } from './NameLayer';
import './composer.css';

export type { Person } from './MentionList';

interface Props {
  draft: string;
  /** The names picked from the list in it: only these count, and only these are blue. */
  picks?: readonly Picked[];
  onDraft: (text: string, picks: Picked[]) => void;
  onSend: () => void;
  /** Set while a turn runs: the button stops it instead of sending. */
  onStop?: () => void;
  inputRef?: Ref<HTMLTextAreaElement>;
  /** Names who gets a message that names nobody: "Message Mira". */
  hint: string;
  /** Who "@" lists, in order; those not in the chat yet last, marked `out`. */
  people?: readonly Person[];
  /** By Send once something is typed: who gets it, and whether it joins the chat. */
  to?: { id: string; name: string; joins?: boolean };
  /** What its field is called: "Message" at the chat's foot, "Reply" at a thread's (0076). */
  label?: string;
  /** Placed at a thread's foot: a cross and Escape close it. */
  onClose?: () => void;
}

const NOBODY: readonly Person[] = [];
const NONE: readonly Picked[] = [];

/**
 * The one composer, on the registry's composer parts. Enter sends, Shift+Enter is a new line; "@" lists names. At the
 * chat's foot it sends main messages and holds Stop; at a thread's foot (0076) it is one row with a cross, and Escape
 * closes it once no "@" list is open.
 */
export function Composer(props: Props) {
  const { draft, picks = NONE, onDraft, onSend, onStop, inputRef, hint, people = NOBODY, to } = props;
  const { label: name = 'Message', onClose } = props;
  const canSend = draft.trim().length > 0;
  const stopping = !!onStop && !canSend;
  // Stop names its scope (the prototype): it ends the chat's turns and the commands they started.
  const label = stopping ? 'Stop this chat now' : 'Send';
  const box = useRef<HTMLTextAreaElement | null>(null);
  const ref = useCallback(
    (el: HTMLTextAreaElement | null) => {
      box.current = el;
      if (typeof inputRef === 'function') inputRef(el);
      else if (inputRef) inputRef.current = el;
    },
    [inputRef],
  );
  // The caret, with the text it was read in: text set from outside (a chat opened, Message) puts it at the end.
  const [read, setRead] = useState({ text: draft, caret: draft.length });
  const caret = read.text === draft ? read.caret : draft.length;
  const [focused, setFocused] = useState(false);
  // The "@" Escape closed, by where it is, until the text has no "@" being typed.
  const [shut, setShut] = useState<number | null>(null);
  const [lit, setLit] = useState({ key: '', at: 0 });
  // Where the caret goes once a picked name is in the text.
  const placed = useRef<number | null>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: a picked name's caret is placed once its text is drawn
  useLayoutEffect(() => {
    if (placed.current === null || !box.current) return;
    box.current.setSelectionRange(placed.current, placed.current);
    placed.current = null;
  }, [draft]);

  const mention = typing(draft, caret);
  const matches = mention ? people.filter((p) => p.name.toLowerCase().startsWith(mention.query)) : [];
  const open = focused && !!mention && mention.at !== shut && matches.length > 0;
  const key = mention ? `${mention.at}:${mention.query}` : '';
  const at = lit.key === key ? lit.at : 0;

  const choose = (k: number) => {
    if (!mention) return;
    const who = matches[k];
    const next = insert(draft, mention.at, caret, who.name);
    placed.current = next.caret;
    setRead(next);
    const kept = follow(draft, next.text, next.caret, picks);
    onDraft(next.text, [...kept, { at: mention.at, id: who.id, name: who.name }]);
  };
  const moved = (e: SyntheticEvent<HTMLTextAreaElement>) =>
    setRead({ text: e.currentTarget.value, caret: e.currentTarget.selectionStart });
  const keyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing) return;
    if (open && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      e.preventDefault();
      const n = matches.length;
      const next = (at + (e.key === 'ArrowDown' ? 1 : n - 1)) % n;
      setLit({ key, at: next });
      document.getElementById(`mention-${matches[next].id}`)?.scrollIntoView({ block: 'nearest' });
      return;
    }
    if (open && (e.key === 'Enter' || e.key === 'Tab') && !e.shiftKey) {
      e.preventDefault();
      return choose(at);
    }
    if (open && e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      return setShut(mention.at);
    }
    if (e.key === 'Escape' && onClose) {
      e.preventDefault();
      e.stopPropagation();
      return onClose();
    }
    if (e.key !== 'Enter' || e.shiftKey) return;
    e.preventDefault();
    if (canSend) onSend();
  };
  return (
    <Frame className="composer" data-thread={onClose ? '' : undefined}>
      {open && <MentionList people={matches} lit={at} onPick={choose} />}
      <ComposerBar>
        <div className="composer-field">
          {people.length > 0 && <NameLayer text={draft} picks={picks} box={box} />}
          <textarea
            ref={ref}
            className={people.length > 0 ? 'composer-input named' : 'composer-input'}
            aria-label={name}
            aria-activedescendant={open ? `mention-${matches[at].id}` : undefined}
            placeholder={hint}
            rows={1}
            value={draft}
            onChange={(e) => {
              const now = { text: e.target.value, caret: e.target.selectionStart };
              if (!typing(now.text, now.caret)) setShut(null);
              setRead(now);
              onDraft(now.text, follow(draft, now.text, now.caret, picks));
            }}
            onSelect={moved}
            onFocus={() => setFocused(true)}
            onBlur={() => setFocused(false)}
            onKeyDown={keyDown}
          />
        </div>
        <ComposerToolbar>
          <ComposerActions />
          <ComposerActions>
            {to && canSend && (
              <span className="composer-to" role="status">
                <Face agent={{ id: to.id, name: to.name }} size={20} />
                {to.joins ? (
                  <span>
                    <b>{to.name}</b> joins
                  </span>
                ) : (
                  <span>
                    To <b>{to.name}</b>
                  </span>
                )}
              </span>
            )}
            {onClose && (
              <IconButton aria-label="Close" className="composer-close" onClick={onClose}>
                <X aria-hidden="true" />
              </IconButton>
            )}
            <ComposerSend
              aria-label={label}
              streaming={stopping}
              idle={!canSend}
              disabled={!canSend && !stopping}
              onClick={stopping ? onStop : onSend}
            />
          </ComposerActions>
        </ComposerToolbar>
      </ComposerBar>
    </Frame>
  );
}
