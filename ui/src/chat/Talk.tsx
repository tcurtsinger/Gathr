import { MessagePrimitive } from '@assistant-ui/react';
import { afterDraw, Foot } from './Foot';
import type { Custom } from './messages';
import { Pieces, parts } from './Replies';
import { AgentFace, Bubble, NameLine, NewWord, Said, When } from './Said';
import { useActions } from './Thread';
import type { Said as Message } from './threads';

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

/**
 * A message between agents (0072), an item of its own: in a thread, smaller, with the @Name of whom it is to. A turn
 * that only sent it has its runs of steps as lines above its words, as a turn's block has.
 */
export function TalkMessage({ own, said: m }: { own: Custom; said: Message }) {
  const actions = useActions();
  const name = m.from ?? 'An agent';
  const { by } = m;
  const small = own.reply;
  return (
    <Said
      root
      reply={small}
      agent={m.agent}
      small={small}
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
          pieces={[
            <When key="t" time={m.time} />,
            small && <NewWord key="new" topic={own.topic} time={own.time} />,
          ]}
        />
      }
    >
      <Bubble
        lead={m.pieces && <Pieces pieces={m.pieces} onOpenTask={actions.openTask} />}
        quote={m.quote}
        mention={m.to}
        foot={<Foot fold={own.thread} agreed={m.agreed} like={m.like} answer={own.answer} />}
      >
        <MessagePrimitive.Parts components={parts} />
      </Bubble>
    </Said>
  );
}
