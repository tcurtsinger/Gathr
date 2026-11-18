import { useAuiState } from '@assistant-ui/react';
import { FileText } from 'lucide-react';
import { useContext } from 'react';
import { GathrFace } from '../components/Face';
import { Asked, QuestionsContext } from './Asked';
import { MainWord } from './MainWord';
import type { EventLine } from './messages';
import { custom } from './Replies';
import { Bubble, NameLine, Said, When } from './Said';
import { useActions } from './Thread';
import { Event } from './TurnParts';

/**
 * What nobody said, said by Gathr as a speaker of its own (0073): its face, its name and the time, and its words in an
 * outlined bubble, one step quieter than an agent's. Its own question sits in that bubble, answered there. Lines that
 * belong to later changes (a task heard back, a plan's word) keep today's line inside the bubble.
 */
export function GathrMessage() {
  const own = useAuiState(custom);
  const asked = own.asked ?? [];
  const events = own.events ?? [];
  const why = useContext(QuestionsContext)?.tasks.find((t) => asked.includes(t.number))?.why;
  const fixed = events.find(isFixed);
  return (
    <Said
      root
      gathr
      reply={own.reply}
      small={own.reply}
      face={<GathrFace size={own.reply ? 28 : 36} />}
      line={
        <NameLine
          name="Gathr"
          pieces={[<When key="t" time={own.time} />]}
          state={<MainWord state={own.state} />}
        />
      }
    >
      <Bubble quote={fixed?.text}>
        {why && <p className="gathr-says">{why}</p>}
        {events.map((e, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: its lines keep their order
          <Says key={i} event={e} />
        ))}
        {asked.map((n) => (
          <Asked key={n} task={n} />
        ))}
      </Bubble>
    </Said>
  );
}

// The service's own word (asks.rs `FIXED`), read by asks.ts `fixed`.
const isFixed = (e: EventLine) => e.kind === 'task' && e.who === 'Fixed:';

function Says({ event }: { event: EventLine }) {
  const actions = useActions();
  const more = event.more?.split('\n').filter(Boolean) ?? [];
  // A note, or a late check's sentence: Gathr's words, whole.
  if (event.kind === 'note' || (event.kind === 'version' && !event.who))
    return <p className="gathr-says">{event.text}</p>;
  if (event.kind === 'changed')
    return (
      <>
        <p className="gathr-says">{event.text}</p>
        <ul className="gathr-list">
          {more.map((file) => (
            <li key={file}>
              <FileText aria-hidden="true" />
              <code>{file}</code>
            </li>
          ))}
        </ul>
      </>
    );
  if (isFixed(event))
    return (
      <>
        <p className="gathr-says">Fixed.</p>
        <ul className="gathr-list" data-tasks="">
          {more.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      </>
    );
  return <Event event={event} onOpenPlan={actions.openPlan} />;
}
