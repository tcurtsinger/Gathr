import { Search } from 'lucide-react';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type ChatHit, type Failure, type MessageHit } from '../api';
import { NavFlyout } from '../components/NavFlyout';
import { FailureNotice } from '../components/Notice';
import { report } from '../report';
import './search.css';

interface Props {
  /** An agent's name by its id. */
  name: (agent: string | null) => string;
  /** Opens the hit's chat, at the message for a message hit; `at` is when the person picked it. */
  onOpen: (hit: ChatHit | MessageHit, at: number) => void;
  onReference: (ref: string) => void;
}

type Found = { chats: ChatHit[]; messages: MessageHit[] };

// The owner's SerenityOS palette waits as long; an answer slower than SLOW shows Searching….
const DEBOUNCE = 150;
const SLOW = 300;
const key = /Mac/.test(navigator.userAgent) ? '⌘K' : 'Ctrl K';

/**
 * Search across chats (plans/search.md, DESIGN.md "Navigation and workspace"): chat titles, then
 * messages, newest first, with one cursor across both. The query stays between openings; an answer to
 * an older query is dropped.
 */
export function SearchFlyout({ name, onOpen, onReference }: Props) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState('');
  const [found, setFound] = useState<Found | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [slow, setSlow] = useState(false);
  const [cursor, setCursor] = useState(-1);
  const [tries, setTries] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const asked = useRef(0);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== 'k') return;
      e.preventDefault();
      setOpen(true);
      input.current?.focus();
      input.current?.select();
    };
    addEventListener('keydown', onKey);
    return () => removeEventListener('keydown', onKey);
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: Retry asks again for the same words
  useEffect(() => {
    const words = q.trim();
    const n = ++asked.current;
    if (!words) {
      setFound(null);
      setFailure(null);
      setSlow(false);
      return;
    }
    const wait = setTimeout(() => {
      const late = setTimeout(() => n === asked.current && setSlow(true), SLOW);
      api
        .search(words)
        .then(
          (answer) => {
            if (n !== asked.current) return;
            setFound(answer);
            setFailure(null);
            setCursor(-1);
          },
          (f: Failure) => {
            if (n !== asked.current) return;
            setFailure(f);
            setFound(null);
            report({ name: 'gathr.ui.search.error', error: 'request', attrs: { ref: f.ref } });
          },
        )
        .finally(() => {
          clearTimeout(late);
          if (n === asked.current) setSlow(false);
        });
    }, DEBOUNCE);
    return () => clearTimeout(wait);
  }, [q, tries]);

  const hits: (ChatHit | MessageHit)[] = found ? [...found.chats, ...found.messages] : [];
  // A message hit takes focus to itself in the chat, not back to Search.
  const toMessage = useRef(false);
  const pick = (hit: ChatHit | MessageHit) => {
    const at = performance.now();
    toMessage.current = 'message' in hit;
    setOpen(false);
    onOpen(hit, at);
  };
  const row = (hit: ChatHit | MessageHit, i: number) => {
    const message = 'message' in hit;
    return (
      <button
        type="button"
        key={message ? hit.message : `chat-${hit.chat}`}
        id={`search-hit-${i}`}
        className="flyout-row"
        role="option"
        aria-selected={i === cursor}
        tabIndex={-1}
        onClick={() => pick(hit)}
      >
        <span className="search-title">
          {message ? hit.chat_title : marked(hit.title, titleMarks(hit.title, q.trim()))}
        </span>
        {message && <span className="search-snippet">{marked(hit.snippet, hit.marks)}</span>}
        <small>
          {message ? `${who(hit, name)} · ${when(hit.time)}` : 'Chat'}
          {hit.archived && <span>Archived</span>}
        </small>
      </button>
    );
  };
  const group = (label: string, list: (ChatHit | MessageHit)[], from: number) =>
    list.length > 0 && (
      // biome-ignore lint/a11y/useSemanticElements: a listbox's groups, as ARIA's combobox pattern has them
      <div role="group" aria-labelledby={`search-${label}`}>
        <h3 id={`search-${label}`}>{label}</h3>
        {list.map((hit, i) => row(hit, from + i))}
      </div>
    );
  const status = !q.trim()
    ? 'Search chats and messages'
    : slow
      ? 'Searching…'
      : found && hits.length === 0
        ? 'No matches'
        : '';

  return (
    <NavFlyout
      title="Search"
      open={open}
      onOpenChange={setOpen}
      focus={() => input.current}
      onCloseAutoFocus={(e) => {
        if (toMessage.current) e.preventDefault();
        toMessage.current = false;
      }}
      trigger={
        <>
          <Search aria-hidden="true" />
          Search
          <kbd className="nav-count">{key}</kbd>
        </>
      }
    >
      <label className="search-field">
        <input
          ref={input}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search chats and messages"
          aria-label="Search chats and messages"
          maxLength={200}
          autoComplete="off"
          role="combobox"
          aria-autocomplete="list"
          aria-expanded={hits.length > 0}
          aria-controls="search-results"
          aria-activedescendant={cursor >= 0 ? `search-hit-${cursor}` : undefined}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && hits[cursor]) {
              e.preventDefault();
              pick(hits[cursor]);
            } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && hits.length) {
              e.preventDefault();
              const next =
                (cursor + (e.key === 'ArrowDown' ? 1 : hits.length - 1) + hits.length) % hits.length;
              setCursor(next);
              document.getElementById(`search-hit-${next}`)?.scrollIntoView({ block: 'nearest' });
            }
          }}
        />
      </label>
      <div className="flyout-body" id="search-results" role="listbox" aria-label="Search results">
        {group('Chats', found?.chats ?? [], 0)}
        {group('Messages', found?.messages ?? [], found?.chats.length ?? 0)}
        {failure && (
          <FailureNotice
            failure={failure}
            onReference={onReference}
            action={
              <Button type="button" variant="outline" onClick={() => setTries((n) => n + 1)}>
                Retry
              </Button>
            }
          />
        )}
        <p className="flyout-empty" role="status" hidden={!status}>
          {status}
        </p>
      </div>
    </NavFlyout>
  );
}

/** "You" for the person's own words, Gathr for what it told an agent, else the agent's name. */
function who(hit: MessageHit, name: (agent: string | null) => string): string {
  return hit.role === 'user' ? 'You' : hit.role === 'gathr' ? 'Gathr' : name(hit.agent);
}

/** The text with each [start, end] (UTF-16 units, as JavaScript counts) in a mark; never HTML. */
function marked(text: string, marks: [number, number][]): ReactNode[] {
  const out: ReactNode[] = [];
  let at = 0;
  for (const [start, end] of marks) {
    if (start < at || end <= start) continue;
    out.push(text.slice(at, start), <mark key={start}>{text.slice(start, end)}</mark>);
    at = end;
  }
  out.push(text.slice(at));
  return out;
}

/** Where a title holds the query, as the service matched it: the whole query, case ignored. */
function titleMarks(title: string, q: string): [number, number][] {
  const at = title.toLowerCase().indexOf(q.toLowerCase());
  return at < 0 || !q ? [] : [[at, at + q.length]];
}

/** A short time: 09:52 today, then Yesterday, a weekday this week, else the date. */
function when(time: string, now = new Date()): string {
  const at = new Date(time);
  const days = Math.round(
    (new Date(now.toDateString()).getTime() - new Date(at.toDateString()).getTime()) / 86_400_000,
  );
  if (days <= 0) return at.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
  if (days === 1) return 'Yesterday';
  if (days < 7) return at.toLocaleDateString([], { weekday: 'long' });
  return at.toLocaleDateString([], { day: 'numeric', month: 'short' });
}
