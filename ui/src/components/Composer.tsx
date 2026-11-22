import { FilePlus, Paperclip, X } from 'lucide-react';
import {
  Fragment,
  type KeyboardEvent,
  type Ref,
  type RefObject,
  type SyntheticEvent,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { createPortal } from 'react-dom';
import {
  ComposerActions,
  ComposerBar,
  ComposerSend,
  ComposerToolbar,
  Composer as Frame,
} from '@/components/assistant-ui/elements/composer';
import { Face } from './Face';
import { FileTiles, LocalTile } from './FileTile';
import { MAX_FILES, MOST } from './files';
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
  /** By Send once something is typed: who gets it, each, and those who join the chat by it (0028). */
  to?: readonly { id: string; name: string; joins?: boolean }[];
  /** What its field is called: "Message" at the chat's foot, "Reply" at a thread's (0076). */
  label?: string;
  /** Placed at a thread's foot: a cross and Escape close it. */
  onClose?: () => void;
  /** The files added to the message, kept until Send: with `onFiles`, the box takes files. */
  files?: readonly File[];
  onFiles?: (files: File[]) => void;
  /** Where a dropped file is taken, besides the box: the chat's column. */
  dropOn?: RefObject<HTMLElement | null>;
  /** Takes nothing: there is nobody to send to. */
  off?: boolean;
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
  const { label: name = 'Message', onClose, files = NO_FILES, onFiles, off } = props;
  const canSend = draft.trim().length > 0 || files.length > 0;
  // Why the last files added weren't all taken, until the next are.
  const [note, setNote] = useState('');
  const add = (more: File[]) => {
    if (!onFiles || !more.length) return;
    const fits = more.filter((f) => f.size <= MOST);
    const room = fits.slice(0, MAX_FILES - files.length);
    setNote(
      fits.length < more.length
        ? 'Files over 10 MB can’t be added'
        : room.length < fits.length
          ? 'A message takes up to 10 files'
          : '',
    );
    if (room.length) onFiles([...files, ...room.map(named)]);
  };
  const send = () => {
    setNote('');
    onSend();
  };
  const dragging = useDrop(onFiles && !off ? props.dropOn : undefined, add);
  const picker = useRef<HTMLInputElement>(null);
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
    if (canSend) send();
  };
  return (
    <Frame className="composer" data-thread={onClose ? '' : undefined}>
      {open && <MentionList people={matches} lit={at} onPick={choose} apart={people.some((p) => !p.out)} />}
      <ComposerBar>
        {files.length > 0 && (
          <FileTiles>
            {files.map((f, i) => (
              <LocalTile
                // biome-ignore lint/suspicious/noArrayIndexKey: the same file may be added twice
                key={i}
                file={f}
                onRemove={() => {
                  setNote('');
                  onFiles?.(files.filter((_, j) => j !== i));
                }}
              />
            ))}
          </FileTiles>
        )}
        {note && (
          <p className="composer-note" role="status">
            {note}
          </p>
        )}
        <div className="composer-field">
          {people.length > 0 && <NameLayer text={draft} picks={picks} box={box} />}
          <textarea
            ref={ref}
            className={people.length > 0 ? 'composer-input named' : 'composer-input'}
            aria-label={name}
            aria-activedescendant={open ? `mention-${matches[at].id}` : undefined}
            placeholder={hint}
            rows={1}
            disabled={off}
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
            onPaste={(e) => {
              if (!onFiles || !e.clipboardData.files.length) return;
              e.preventDefault();
              add([...e.clipboardData.files]);
            }}
          />
        </div>
        <ComposerToolbar>
          <ComposerActions>
            {onFiles && (
              <>
                <IconButton
                  aria-label="Add files"
                  className="composer-attach"
                  disabled={off}
                  onClick={() => picker.current?.click()}
                >
                  <Paperclip aria-hidden="true" />
                </IconButton>
                <input
                  ref={picker}
                  type="file"
                  multiple
                  hidden
                  aria-label="Files"
                  onChange={(e) => {
                    add([...(e.target.files ?? [])]);
                    e.target.value = '';
                  }}
                />
              </>
            )}
          </ComposerActions>
          <ComposerActions>
            {!!to?.length && canSend && (
              <span className="composer-to" role="status">
                <span className="composer-faces">
                  {to.map((p) => (
                    <Face key={p.id} agent={{ id: p.id, name: p.name }} size={20} />
                  ))}
                </span>
                <span className="composer-who">{sign(to)}</span>
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
              disabled={!stopping && (off || !canSend)}
              onClick={stopping ? onStop : send}
            />
          </ComposerActions>
        </ComposerToolbar>
      </ComposerBar>
      {dragging &&
        props.dropOn?.current &&
        createPortal(
          <div className="file-drop">
            <FilePlus aria-hidden="true" />
            Drop to add to your message
          </div>,
          props.dropOn.current,
        )}
    </Frame>
  );
}

const NO_FILES: readonly File[] = [];

/** A pasted picture with no name gets a plain one. */
function named(file: File) {
  if (file.name) return file;
  const ext = file.type.split('/')[1] ?? 'png';
  return new File([file], `picture.${ext === 'jpeg' ? 'jpg' : ext}`, { type: file.type });
}

/** Whether files are dragged over `target`; a drop there gives them to `take`. */
function useDrop(target: RefObject<HTMLElement | null> | undefined, take: (files: File[]) => void) {
  const [over, setOver] = useState(false);
  const taking = useRef(take);
  taking.current = take;
  useEffect(() => {
    const el = target?.current;
    if (!el) return;
    const carries = (e: DragEvent) => !!e.dataTransfer?.types.includes('Files');
    const enter = (e: DragEvent) => {
      if (!carries(e)) return;
      e.preventDefault();
      setOver(true);
    };
    const leave = (e: DragEvent) => {
      if (!el.contains(e.relatedTarget as Node | null)) setOver(false);
    };
    const drop = (e: DragEvent) => {
      if (!carries(e)) return;
      e.preventDefault();
      setOver(false);
      taking.current([...(e.dataTransfer?.files ?? [])]);
    };
    el.addEventListener('dragenter', enter);
    el.addEventListener('dragover', enter);
    el.addEventListener('dragleave', leave);
    el.addEventListener('drop', drop);
    return () => {
      el.removeEventListener('dragenter', enter);
      el.removeEventListener('dragover', enter);
      el.removeEventListener('dragleave', leave);
      el.removeEventListener('drop', drop);
    };
  }, [target]);
  return over;
}

/** "Ann", "Ann and Bo", "Ann, Bo and Cy": each name bold. */
function names(people: readonly { id: string; name: string }[]) {
  return people.map((p, i) => (
    <Fragment key={p.id}>
      {i === 0 ? '' : i === people.length - 1 ? ' and ' : ', '}
      <b>{p.name}</b>
    </Fragment>
  ));
}

/** "To Ann and Bo"; in a chat with no project, "Bo joins" or "To Ann, Bo and Cy join". */
function sign(to: readonly { id: string; name: string; joins?: boolean }[]) {
  const staying = to.filter((p) => !p.joins);
  const joining = to.filter((p) => p.joins);
  return (
    <>
      {staying.length > 0 && <>To {names(staying)}</>}
      {staying.length > 0 && joining.length > 0 && ', '}
      {joining.length > 0 && (
        <>
          {names(joining)} {joining.length > 1 ? 'join' : 'joins'}
        </>
      )}
    </>
  );
}
