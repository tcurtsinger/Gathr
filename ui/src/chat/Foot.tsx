import { ChevronRight } from 'lucide-react';
import { MessageActions } from '../components/MessageActions';
import type { Like } from './likes';
import type { Answer, Fold } from './messages';
import { liking } from './Replies';
import { useUnseen } from './reads';
import { revealOpened } from './reveal';
import { Reactions } from './Said';
import { useActions } from './Thread';
import { useReply } from './ThreadBox';

/**
 * The row under a message's bubble (0076, the owner's call): its thread's fold line on a main message with replies,
 * the thumbs-up count, then Reply and Like at the right. The buttons show while the message is pointed at or holds
 * keyboard focus, in a spot always kept for them, so nothing moves when they do; they end at the bubble's bottom right
 * edge, or after the row's own content when the bubble is narrower (said.css). None when the message has nothing to
 * put there.
 */
export function Foot({
  fold,
  agreed,
  like,
  answer,
}: {
  fold?: Fold;
  agreed?: string[];
  like?: Like;
  answer?: Answer;
}) {
  const actions = useActions();
  const liked = liking(like, actions);
  const reply = useReply(answer);
  if (!fold && !agreed?.length && !liked && !reply) return null;
  return (
    <div className="said-foot">
      {fold && <FoldLine fold={fold} />}
      {!!agreed?.length && <Reactions names={agreed} mine={liked?.given} onToggle={liked?.onToggle} />}
      <MessageActions like={liked} reply={reply} />
    </div>
  );
}

/**
 * A main message's fold line (0075): folded, how many replies, or in the attention blue how many are new (0077); open,
 * "Collapse replies" (the owner's words). A press folds or opens the thread; the line stays where it was, so nothing
 * above it moves, and what opens at the foot of the chat scrolls up to show above the message box.
 */
function FoldLine({ fold }: { fold: Fold }) {
  const actions = useActions();
  const { count, open, topic } = fold;
  const fresh = useUnseen(topic, open ? undefined : fold.times);
  return (
    <button
      type="button"
      className="fold-line"
      aria-expanded={open}
      data-new={fresh > 0 || undefined}
      onClick={(e) => {
        const line = e.currentTarget;
        const item = line.closest('.thread-messages > *') as Element;
        const viewport = line.closest<HTMLElement>('.thread-viewport');
        const before = line.getBoundingClientRect().top;
        actions.fold?.(topic);
        // Held where it was by hand, not by the browser's scroll anchoring, which WebKit doesn't have; then, opened,
        // its last reply shown above the message box as far as the line stays in view.
        // The runtime draws the change a moment later: what follows the line changes when it has.
        afterDraw(item.parentElement, () => {
          if (viewport) viewport.scrollTop += line.getBoundingClientRect().top - before;
          if (!open) revealOpened(line, lastOf(item));
        });
      }}
    >
      <ChevronRight className="fold-chevron" aria-hidden="true" />
      {open ? 'Collapse replies' : fresh ? replies(fresh, ' new') : replies(count)}
    </button>
  );
}

/** Runs `then` once the chat's list of items next changes, or never if it doesn't within a second. */
export function afterDraw(list: Element | null, then: () => void) {
  if (!list) return;
  const drawn = new MutationObserver(() => {
    drawn.disconnect();
    then();
  });
  drawn.observe(list, { childList: true });
  setTimeout(() => drawn.disconnect(), 1000);
}

/** The last item of the thread under the main message `item`: its replies follow it, each marked as the thread's. */
function lastOf(item: Element): Element {
  let last = item;
  for (
    let next = item.nextElementSibling;
    next?.matches('[data-reply], [data-thread-item]');
    next = next.nextElementSibling
  )
    last = next;
  return last;
}

const replies = (n: number, what = '') => (n === 1 ? `1${what} reply` : `${n}${what} replies`);
