import { MessagePrimitive, type ThreadMessageLike } from '@assistant-ui/react';
import { ChevronRight, ThumbsUp } from 'lucide-react';
import { createContext, useContext } from 'react';
import { cn } from '@/lib/utils';
import type { Custom, ThreadHead } from './messages';
import './team.css';

/** Which threads the person opened or folded themselves, and how to open one from its link. */
export interface Folding {
  chosen: Map<string, boolean>;
  set: (root: string, open: boolean) => void;
  /** Opens the thread, brings its disclosure into view and moves focus to it. */
  reveal: (root: string) => void;
}

export const FoldingContext = createContext<Folding>({ chosen: new Map(), set: () => {}, reveal: () => {} });

// The person's own choice for each thread outlives a switch to another chat (DESIGN.md: retain it).
let remembered = new Map<string, boolean>();

export const rememberedFolds = () => remembered;

export function remember(root: string, open: boolean) {
  remembered = new Map(remembered).set(root, open);
  return remembered;
}

/** Open while its round runs, folded once the lead's last word is in, unless the person chose. */
export const isOpen = (head: ThreadHead, chosen: Map<string, boolean>) => chosen.get(head.root) ?? !head.done;

const customOf = (m: ThreadMessageLike) => (m.metadata?.custom ?? {}) as Custom;

/** The thread without the replies of folded threads. */
export function unfolded(all: ThreadMessageLike[], chosen: Map<string, boolean>): ThreadMessageLike[] {
  const closed = new Set<string>();
  for (const m of all) {
    const { head } = customOf(m);
    if (head && !isOpen(head, chosen)) closed.add(head.root);
  }
  if (closed.size === 0) return all;
  return all.filter((m) => !closed.has(customOf(m).thread ?? ''));
}

export const headId = (root: string) => `thread-${root}`;

/** An item's classes in a thread: indented on its hairline, which starts and ends with the thread. */
export const threaded = ({ thread, first, end }: Custom) =>
  cn(thread && 'in-thread', first && 'thread-first', end && 'thread-end');

export function TeamHead({ head }: { head: ThreadHead }) {
  const folding = useContext(FoldingContext);
  const open = isOpen(head, folding.chosen);
  return (
    <MessagePrimitive.Root className="team-head" data-closed={!open || head.alone || undefined}>
      <button
        type="button"
        id={headId(head.root)}
        aria-expanded={open}
        onClick={() => folding.set(head.root, !open)}
      >
        <ChevronRight className="team-head-chevron" aria-hidden />
        <strong>Team thread</strong>
        <span className="team-head-state">{head.text}</span>
      </button>
    </MessagePrimitive.Root>
  );
}

/** What Gathr told an agent: a member's task above its work, else a wake-up in Gathr's own voice. */
export function GathrLine({ custom }: { custom: Custom }) {
  const { assignment } = custom;
  if (assignment) {
    const who = assignment.from && assignment.to ? `${assignment.from} to ${assignment.to}` : assignment.to;
    return (
      <MessagePrimitive.Root className={cn('assignment', threaded(custom))}>
        {[who, assignment.what].filter(Boolean).join(' · ')}
      </MessagePrimitive.Root>
    );
  }
  return (
    <MessagePrimitive.Root className={cn('gathr-line', threaded(custom))}>
      <span className="gathr-name">Gathr</span>
      <span className="gathr-text">{custom.gathr}</span>
    </MessagePrimitive.Root>
  );
}

/** Under the lead's last word on a round: the link back to its thread, and who agreed. */
export function LastWord({ back, agreed }: { back: { root: string; done: boolean }; agreed: string[] }) {
  const folding = useContext(FoldingContext);
  // A disclosure, as it opens the thread: assistant-ui stops following the bottom for it.
  return (
    <div className="last-word">
      <button
        type="button"
        className="link thread-link"
        aria-expanded={folding.chosen.get(back.root) ?? !back.done}
        onClick={() => folding.reveal(back.root)}
      >
        View team thread
      </button>
      {agreed.map((name) => (
        <span
          key={name}
          className="reaction"
          role="img"
          aria-label={`${name} agrees`}
          title={`${name} agrees`}
        >
          <ThumbsUp aria-hidden />
          {name}
        </span>
      ))}
    </div>
  );
}
