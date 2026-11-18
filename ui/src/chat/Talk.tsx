import { MessagePrimitive } from '@assistant-ui/react';
import { ChevronRight } from 'lucide-react';
import { MessageActions } from '../components/MessageActions';
import type { Custom, Fold } from './messages';
import { liking, parts } from './Replies';
import { revealOpened } from './reveal';
import { AgentFace, Bubble, NameLine, Reactions, Said, When } from './Said';
import { StepsButton, useStepsOpen } from './Steps';
import { useActions } from './Thread';
import { useReply } from './ThreadBox';
import { WorkRow } from './TurnParts';
import type { Said as Message } from './threads';

/**
 * The one row under a main message (0075): the thread's fold line, then the main message's thumbs-up. Folded, the
 * line says how many replies; open, "Collapse replies" (the owner's words). A press folds or opens the thread; the
 * line stays where it was, so nothing above it moves, and what opens at the foot of the chat scrolls up to show above
 * the message box. A main message with a thumbs-up and no replies has the thumbs-up alone.
 */
export function FoldLine({ fold }: { fold: Fold }) {
  const actions = useActions();
  const { agreed, count, open, topic } = fold;
  const like = liking(fold.like, actions);
  const replies = count === 1 ? '1 reply' : `${count} replies`;
  const thumbs = !!agreed?.length && (
    <Reactions names={agreed} mine={like?.given} onToggle={like?.onToggle} />
  );
  // Reply on the thread's line (0076): a plain word after the thumbs-up, answering the main message.
  const reply = useReply(fold.answer);
  const replying = reply && (
    <button type="button" className="fold-reply" onClick={(e) => reply(e.currentTarget)}>
      Reply
    </button>
  );
  if (!count)
    return (
      <MessagePrimitive.Root className="fold" data-thread-item="" data-bare="">
        {thumbs}
        {replying}
      </MessagePrimitive.Root>
    );
  return (
    <MessagePrimitive.Root className="fold" data-thread-item="" data-open={open || undefined}>
      <button
        type="button"
        className="fold-line"
        aria-expanded={open}
        onClick={(e) => {
          const line = e.currentTarget;
          const item = line.closest('.fold') as Element;
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
        {open ? 'Collapse replies' : replies}
      </button>
      {thumbs}
      {replying}
    </MessagePrimitive.Root>
  );
}

/** Runs `then` once the chat's list of items next changes, or never if it doesn't within a second. */
function afterDraw(list: Element | null, then: () => void) {
  if (!list) return;
  const drawn = new MutationObserver(() => {
    drawn.disconnect();
    then();
  });
  drawn.observe(list, { childList: true });
  setTimeout(() => drawn.disconnect(), 1000);
}

/** The last item of a thread whose fold line is `item`: its replies follow it, each marked as the thread's. */
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

/** "Show N earlier replies" above a long thread's newest; pressed, they show and focus goes to the first of them. */
export function Earlier({ earlier }: { earlier: NonNullable<Custom['earlier']> }) {
  const actions = useActions();
  const { n, topic } = earlier;
  return (
    <MessagePrimitive.Root className="earlier" data-thread-item="">
      <button
        type="button"
        className="earlier-button"
        onClick={(e) => {
          const list = e.currentTarget.closest('.thread-messages');
          const at = list ? [...list.children].indexOf(e.currentTarget.closest('.earlier') as Element) : -1;
          // The first reply shown takes this button's place, and focus with it, in reading order.
          afterDraw(list, () => {
            const first = list?.children[at] as HTMLElement | undefined;
            if (!first) return;
            first.tabIndex = -1;
            first.focus();
          });
          actions.earlier?.(topic);
        }}
      >
        {n === 1 ? 'Show 1 earlier reply' : `Show ${n} earlier replies`}
      </button>
    </MessagePrimitive.Root>
  );
}

/** A message between agents (0072), an item of its own: in a thread, smaller, with the @Name of whom it is to. */
export function TalkMessage({ own, said: m }: { own: Custom; said: Message }) {
  const actions = useActions();
  const [open, setOpen] = useStepsOpen(m.work);
  const name = m.from ?? 'An agent';
  const { by, work } = m;
  const like = liking(m.like, actions);
  const reply = useReply(own.answer);
  const small = own.reply;
  return (
    <Said
      root
      reply={small}
      agent={m.agent}
      small={small}
      keep={!!work && (work.running || !!work.failed)}
      face={
        <AgentFace
          agent={m.agent}
          name={name}
          message={m.id}
          size={small ? 28 : 36}
          onOpen={by ? () => actions.openAgent(by) : undefined}
        />
      }
      line={
        <NameLine
          name={name}
          pieces={[<When key="t" time={m.time} />]}
          end={work && <StepsButton work={work} open={open} onOpen={setOpen} />}
        />
      }
    >
      {work && (open || work.running) && (
        <WorkRow work={work} onOpenTask={actions.openTask} open={open} onOpenChange={setOpen} />
      )}
      <Bubble quote={m.quote} mention={m.to} actions={<MessageActions like={like} reply={reply} />}>
        <MessagePrimitive.Parts components={parts} />
      </Bubble>
      {/* A main message's thumbs-up is on the row under it, with its fold line. */}
      {m.agreed.length > 0 && !(own.topic && !own.reply) && (
        <Reactions names={m.agreed} mine={like?.given} onToggle={like?.onToggle} />
      )}
    </Said>
  );
}
