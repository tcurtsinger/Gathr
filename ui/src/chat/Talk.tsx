import { MessagePrimitive } from '@assistant-ui/react';
import { ChevronRight } from 'lucide-react';
import { useState } from 'react';
import { FaceStack } from '../components/Face';
import { MessageActions } from '../components/MessageActions';
import { liking, parts, Reply } from './Replies';
import { revealOpened } from './reveal';
import { AgentFace, Bubble, NameLine, Reactions, Said, When } from './Said';
import { StepsButton, useStepsOpen } from './Steps';
import { useActions } from './Thread';
import { WorkRow } from './TurnParts';
import type { Said as Message } from './threads';

/**
 * Talk between agents (0072), or the work on a PR under its message. Under the message it belongs to, it is a thread:
 * a thin line, smaller faces, and a toggle that folds it to its faces and how many replies. Open while anything in it
 * is open, it folds once everything is settled. With no message to sit under, its messages stand as any other, where
 * the talk began.
 */
export function Talk({
  said,
  under,
  settled = false,
}: {
  said: Message[];
  under: boolean;
  settled?: boolean;
}) {
  const [open, setOpen] = useState(!settled);
  const [was, setWas] = useState(settled);
  if (settled !== was) {
    setWas(settled);
    setOpen(!settled);
  }
  const messages = said.map((m, index) =>
    m.block ? (
      <Reply key={m.id} own={m.block} id={m.id} index={index} small={under} />
    ) : (
      <TalkMessage key={m.id} m={m} index={index} small={under} />
    ),
  );
  if (!under) return <MessagePrimitive.Root className="talk">{messages}</MessagePrimitive.Root>;
  const who = [...new Map(said.flatMap((m) => (m.agent ? [[m.agent, m.from ?? '']] : []))).entries()];
  return (
    <MessagePrimitive.Root className="talk" data-thread="" data-open={open || undefined}>
      <button
        type="button"
        className="talk-toggle"
        aria-expanded={open}
        onClick={(e) => {
          setOpen(!open);
          const thread = e.currentTarget.closest('.talk');
          if (!open && thread) revealOpened(e.currentTarget, thread);
        }}
      >
        <ChevronRight className="talk-chevron" aria-hidden="true" />
        {open ? (
          'Hide'
        ) : (
          <>
            <FaceStack agents={who.map(([agent, name]) => ({ agent, name }))} />
            {said.length === 1 ? '1 reply' : `${said.length} replies`}
          </>
        )}
      </button>
      {open && (
        <section className="talk-replies" aria-label="Replies">
          {messages}
        </section>
      )}
    </MessagePrimitive.Root>
  );
}

function TalkMessage({ m, index, small }: { m: Message; index: number; small: boolean }) {
  const actions = useActions();
  const [open, setOpen] = useStepsOpen(m.work);
  const name = m.from ?? 'An agent';
  const { by, work } = m;
  const like = liking(m.like, actions);
  return (
    <Said
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
      <Bubble quote={m.quote} mention={m.to} actions={like && <MessageActions like={like} />}>
        <MessagePrimitive.PartByIndex index={index} components={parts} />
      </Bubble>
      {m.agreed.length > 0 && <Reactions names={m.agreed} mine={like?.given} onToggle={like?.onToggle} />}
    </Said>
  );
}
