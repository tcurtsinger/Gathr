import { MessagePrimitive } from '@assistant-ui/react';
import {
  Bell,
  Check,
  ChevronRight,
  CircleCheck,
  CircleHelp,
  LoaderIcon,
  Minus,
  PenLine,
  X,
} from 'lucide-react';
import { createContext, type ReactNode, useContext, useLayoutEffect, useRef, useState } from 'react';
import {
  CollapsibleContent,
  CollapsibleRoot,
  CollapsibleTrigger,
} from '@/components/assistant-ui/elements/collapsible-root';
import { SpeakerIdentity } from '@/components/assistant-ui/elements/speaker-identity';
import { ToolGroupContent, ToolGroupRoot } from '@/components/assistant-ui/elements/tool-group.aui';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import type { Step, Work } from './fold';
import type { Custom, EventLine } from './messages';
import { clock } from './team';
import { Elapsed } from './Working';
import './team.css';

/** The text a search hit opened at. */
export const FoundContext = createContext<string | undefined>(undefined);

/** Who a block is from, and when: "Cass · on Open the settings tools PR", "Juno · to Orin". */
export function Speaker({
  name,
  detail,
  time,
  onOpen,
}: {
  name: string;
  detail?: string;
  time?: string;
  /** In a team chat: its initial opens the agent's panel. */
  onOpen?: () => void;
}) {
  return (
    <header className="speaker">
      <span className="speaker-who">
        <SpeakerIdentity
          className="w-auto min-w-0"
          turns={[{ id: 'author', kind: 'agent', name, detail, text: '' }]}
        />
        {/* Over the registry's avatar, which takes no handler of its own. */}
        {onOpen && (
          <button type="button" className="speaker-open" aria-label={`Open ${name}`} onClick={onOpen} />
        )}
      </span>
      {time && <time dateTime={time}>{clock(time)}</time>}
    </header>
  );
}

/**
 * What a turn did before its message, as one row (0037): closed, how long it worked and how much it did,
 * or what it's doing now while it runs; open, each step and what it wrote between them.
 */
export function WorkRow({
  work,
  onOpenTask,
  startOpen = false,
}: {
  work: Work;
  onOpenTask: (task: number) => void;
  /** Its steps show from the start, as in the agent's panel. */
  startOpen?: boolean;
}) {
  const found = useContext(FoundContext);
  // A search hit on something it wrote mid-work opens the row at it.
  const holds = !!found && work.steps.some((s) => 'say' in s && s.say === found);
  const [open, setOpen] = useState(holds || startOpen);
  const [held, setHeld] = useState(holds);
  if (holds !== held) {
    setHeld(holds);
    if (holds) setOpen(true);
  }
  return (
    <ToolGroupRoot variant="ghost" className="work" open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger className="work-row" data-running={work.running || undefined}>
        <ChevronRight className="work-chevron" aria-hidden="true" />
        <span>
          <span className={work.running ? 'shimmer motion-reduce:animate-none' : undefined}>
            {work.label}
          </span>
          <span className="work-notes">
            {work.notes.map((note) => ` · ${note}`)}
            {work.running && work.since !== undefined && <Elapsed since={work.since} lead=" " />}
          </span>
        </span>
      </CollapsibleTrigger>
      {/*
        Opened for a hit, it's whole and in place at once: the hit is centred by where it is. The registry
        also slides the steps in from 4px above, which moved the hit after it was centred.
      */}
      <ToolGroupContent className={holds ? 'animate-none! [&>*>*]:animate-none!' : undefined}>
        <TooltipProvider>
          <ol className="work-steps">
            {work.steps.map((step) => (
              <StepLine key={step.id} step={step} found={found} onOpenTask={onOpenTask} />
            ))}
          </ol>
        </TooltipProvider>
      </ToolGroupContent>
    </ToolGroupRoot>
  );
}

const MARK = { running: LoaderIcon, ok: Check, error: X, stopped: Minus };

function StepLine({
  step,
  found,
  onOpenTask,
}: {
  step: Step;
  found?: string;
  onOpenTask: (task: number) => void;
}) {
  if ('say' in step)
    return <li className={step.say === found ? 'work-say found-part' : 'work-say'}>{step.say}</li>;
  const Mark = MARK[step.state];
  const { task } = step;
  return (
    <li data-state={step.state}>
      <Mark
        aria-hidden="true"
        className={
          step.state === 'running' ? 'work-mark animate-spin motion-reduce:animate-none' : 'work-mark'
        }
      />
      <span className="work-label">{step.label}</span>
      {step.state === 'error' && <span className="work-failed">Failed</span>}
      {step.detail &&
        (task === undefined ? (
          <Ran text={step.detail} />
        ) : (
          <button type="button" className="work-detail" onClick={() => onOpenTask(task)}>
            {step.detail}
          </button>
        ))}
      {step.time && <span className="work-time">{step.time}</span>}
    </li>
  );
}

/**
 * The chip for what a step ran (0041), in the step's one line. Cut short, it shows whole when pointed at
 * or focused: a cut chip is a tab stop, so the keyboard reaches its text; a whole one is not.
 */
function Ran({ text }: { text: string }) {
  const chip = useRef<HTMLSpanElement>(null);
  const [cut, setCut] = useState(false);
  const [whole, setWhole] = useState(false);
  useLayoutEffect(() => {
    const el = chip.current;
    if (!el) return;
    const measure = () => setCut(el.scrollWidth > el.clientWidth);
    measure();
    const watch = new ResizeObserver(measure);
    watch.observe(el);
    return () => watch.disconnect();
  }, []);
  return (
    <Tooltip open={whole} onOpenChange={(open) => setWhole(open && cut)}>
      <TooltipTrigger asChild>
        <span ref={chip} className="work-detail" data-ran="" tabIndex={cut ? 0 : undefined}>
          {text}
        </span>
      </TooltipTrigger>
      <TooltipContent className="work-ran-whole">{text}</TooltipContent>
    </Tooltip>
  );
}

/** Messages between agents, on one surface, so talk reads as talk; `text` draws the nth one's words. */
export function Exchange({
  said,
  text,
  onOpenAgent,
}: {
  said: NonNullable<Custom['exchange']>;
  text: (index: number) => ReactNode;
  onOpenAgent: (agent: string) => void;
}) {
  return (
    <MessagePrimitive.Root className="exchange">
      {said.map(({ by, ...m }, index) => (
        <div className="reply" data-speaker="" key={m.id}>
          <Speaker
            name={m.from ?? 'An agent'}
            detail={m.to && `to ${m.to}`}
            time={m.time}
            onOpen={by ? () => onOpenAgent(by) : undefined}
          />
          <div className="reply-body">{text(index)}</div>
        </div>
      ))}
    </MessagePrimitive.Root>
  );
}

const ICON = { task: CircleCheck, answer: PenLine, question: CircleHelp, note: Bell };

/** Gathr's notes that belong to no turn's row, one line each. */
export function Events({ events, onOpenTask }: { events: EventLine[]; onOpenTask: (task: number) => void }) {
  return (
    <MessagePrimitive.Root className="events">
      {events.map((e, index) => (
        // Two lines can read the same ("You answered Orin: Done"): its task tells them apart, else its place.
        <Event key={e.task ?? `at-${index}`} event={e} onOpenTask={onOpenTask} />
      ))}
    </MessagePrimitive.Root>
  );
}

/**
 * One line, cut with … when long; a click shows it whole, with what more there is (ux-issues 27) and
 * its task, which opens in the Tasks tab (room task 70).
 */
function Event({ event, onOpenTask }: { event: EventLine; onOpenTask: (task: number) => void }) {
  const { who, text, more, task } = event;
  const Icon = ICON[event.kind];
  return (
    <CollapsibleRoot>
      <CollapsibleTrigger className="event-line">
        <Icon className="event-icon" aria-hidden="true" />
        <span className="event-text">
          {who && <b>{who} </b>}
          {text}
        </span>
      </CollapsibleTrigger>
      {(more || task !== undefined) && (
        <CollapsibleContent className="event-more">
          {more && <span>{more}</span>}
          {task !== undefined && (
            <button type="button" className="link" onClick={() => onOpenTask(task)}>
              Open task
            </button>
          )}
        </CollapsibleContent>
      )}
    </CollapsibleRoot>
  );
}
