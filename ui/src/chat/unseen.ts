import type { ThreadMessageLike } from '@assistant-ui/react';
import { useEffect, useRef, useState } from 'react';

// The team replies each chat has shown, kept while the window is open, as other chats are opened.
const seen = new Map<string, Set<string>>();
const none: ReadonlySet<string> = new Set();

/**
 * Team replies that came in while the team column was hidden: the header's dot until it opens, then a
 * dot by each one until it's hidden again. What a chat held when it first loaded counts as seen, and so
 * does what comes in while the column shows.
 */
export function useUnseen(chat: string, team: ThreadMessageLike[], open: boolean, loaded: boolean) {
  const replies = team.filter((m) => m.role === 'assistant').map((m) => m.id ?? '');
  if (loaded && chat && !seen.has(chat)) seen.set(chat, new Set(replies));
  const known = seen.get(chat);
  const unseen = known ? replies.filter((id) => !known.has(id)) : [];
  const [highlight, setHighlight] = useState(none);
  const wasOpen = useRef(open);
  const fresh = unseen.join(' ');
  useEffect(() => {
    const opening = open && !wasOpen.current;
    wasOpen.current = open;
    if (!open) return setHighlight(none);
    const marked = fresh ? fresh.split(' ') : [];
    for (const id of marked) known?.add(id);
    if (opening) setHighlight(marked.length ? new Set(marked) : none);
  }, [open, fresh, known]);
  return { dot: !open && unseen.length > 0, highlight };
}
