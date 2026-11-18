import { MessagePrimitive } from '@assistant-ui/react';
import {
  Bell,
  CircleCheck,
  CircleHelp,
  ClipboardList,
  FilePen,
  PenLine,
  Send,
  ThumbsUp,
  Wrench,
} from 'lucide-react';
import { createContext, type ReactNode, useContext, useState } from 'react';
import {
  CollapsibleContent,
  CollapsibleRoot,
  CollapsibleTrigger,
} from '@/components/assistant-ui/elements/collapsible-root';
import { RunActivity, type RunActivityEntry } from '@/components/assistant-ui/elements/run-activity';
import { ToolCall } from '@/components/assistant-ui/elements/tool-call';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { Face } from '../components/Face';
import { counted } from '../plans/plans';
import { useMark } from './faceMarks';
import type { Step, Work } from './fold';
import type { Custom, EventLine } from './messages';
import { SizeChoice } from './Size';
import { clock, list } from './team';
import { Elapsed } from './Working';
import './team.css';
import '../plans/plans.css';

/** The text a search hit opened at. */
export const FoundContext = createContext<string | undefined>(undefined);

/** Who a block is from, and when: "Cass · on Open the settings tools PR", "Juno · to Orin". */
export function Speaker({
  agent,
  name,
  detail,
  time,
  message,
  onOpen,
}: {
  /** The agent's id, for its face. */
  agent?: string;
  name: string;
  detail?: string;
  time?: string;
  /** The message it heads: on the agent's latest, its Working mark turns (0071). */
  message?: string;
  /** In a team chat: its face opens the agent's panel. */
  onOpen?: () => void;
}) {
  const face = <Face agent={{ id: agent ?? '', name }} size={24} mark={useMark(agent, name, message)} />;
  return (
    <header className="speaker">
      <span className="speaker-who">
        {onOpen ? (
          <button type="button" className="speaker-open" aria-label={`Open ${name}`} onClick={onOpen}>
            {face}
          </button>
        ) : (
          face
        )}
        <span className="speaker-line">
          <span className="speaker-name">{name}</span>
          {detail && <span className="speaker-detail">{detail}</span>}
        </span>
      </span>
      {time && <time dateTime={time}>{clock(time)}</time>}
    </header>
  );
}

/**
 * What a turn did before its message (0037), as assistant-ui's Run activity (the owner's call, 2026-11-12):
 * closed, how long it worked and how much it did, or "Working" with its latest step under it while it runs;
 * open, each tool step as a Tool call, which opens to what it ran and what came back (0044), and Gathr's own
 * lines between them: what it wrote mid-work, its thoughts, the tasks it gave.
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
  const entries: RunActivityEntry[] = work.steps.map((step) => ({
    id: step.id,
    kind: 'say' in step ? 'commentary' : 'tool',
    // The line a running row shows under "Working": its latest step, as it reads now.
    label:
      'say' in step
        ? ''
        : [step.state === 'running' ? (step.doing ?? step.label) : step.label, step.detail]
            .filter(Boolean)
            .join(' '),
    content: <StepEntry step={step} found={found} onOpenTask={onOpenTask} />,
  }));
  return (
    <RunActivity
      className="work"
      status={work.running ? 'running' : 'complete'}
      statusLabel={work.running ? 'Working' : work.label}
      durationLabel={
        <span className="work-notes">
          {work.notes.map((note) => `· ${note}`).join(' ')}
          {work.running && work.since !== undefined && <Elapsed since={work.since} lead=" " />}
        </span>
      }
      entries={entries}
      open={open}
      onOpenChange={setOpen}
    />
  );
}

/** One step inside the row: a Tool call, or one of Gathr's own lines. */
function StepEntry({
  step,
  found,
  onOpenTask,
}: {
  step: Step;
  found?: string;
  onOpenTask: (task: number) => void;
}) {
  const [open, setOpen] = useState(false);
  if ('say' in step)
    return <p className={step.say === found ? 'work-say found-part' : 'work-say'}>{step.say}</p>;
  const { task } = step;
  if (task !== undefined)
    return (
      <span className="work-line">
        <Send aria-hidden="true" />
        <span>{step.label}</span>
        <button type="button" className="work-task" onClick={() => onOpenTask(task)}>
          {step.detail}
        </button>
      </span>
    );
  // A thought: how long, never its text (0042), with nothing to open.
  if (step.doing === undefined)
    return (
      <span className="work-line">
        <span className={step.state === 'running' ? 'shimmer motion-reduce:animate-none' : undefined}>
          {step.label}
        </span>
      </span>
    );
  return (
    <ToolCall
      className="work-step"
      label={step.label}
      activeLabel={step.doing}
      query={step.detail ?? ''}
      request={step.request ?? step.label}
      result={step.result ?? (step.state === 'running' ? '' : 'Nothing to show')}
      running={step.state === 'running'}
      failed={step.state === 'error'}
      stopped={step.state === 'stopped'}
      open={open}
      onOpenChange={setOpen}
    />
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
            agent={m.agent}
            name={m.from ?? 'An agent'}
            detail={m.to && `to ${m.to}`}
            time={m.time}
            message={m.id}
            onOpen={by ? () => onOpenAgent(by) : undefined}
          />
          <div className="reply-body">{text(index)}</div>
          {m.agreed.length > 0 && <Agreed names={m.agreed} />}
        </div>
      ))}
    </MessagePrimitive.Root>
  );
}

/** Agents' thumbs-ups on the words above: how many, and who in the tooltip, on hover or focus. */
export function Agreed({ names }: { names: string[] }) {
  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <button type="button" className="agreed" aria-label={`Thumbs-up from ${list(names)}`}>
            <ThumbsUp aria-hidden="true" />
            {names.length}
          </button>
        </TooltipTrigger>
        {/* Under the mark, so it never covers the words it is about. */}
        <TooltipContent side="bottom" align="start" collisionPadding={8}>
          {list(names)}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}

/** A version of the lead's plan in its turn (0062): it opens the plan's tab there. */
export function PlanChip({ version, parts, onOpen }: { version: number; parts: number; onOpen: () => void }) {
  return (
    <button
      type="button"
      className="plan-chip"
      aria-label={`Plan · version ${version}, ${counted(parts, 'part')}`}
      onClick={onOpen}
    >
      <ClipboardList aria-hidden="true" />
      <span>Plan · version {version}</span>
      <small>{counted(parts, 'part')}</small>
    </button>
  );
}

const ICON = {
  task: CircleCheck,
  answer: PenLine,
  question: CircleHelp,
  note: Bell,
  fix: Wrench,
  plan: ClipboardList,
  version: ClipboardList,
  changed: FilePen,
};

/** Gathr's notes that belong to no turn's row, one line each. */
export function Events({
  events,
  onOpenTask,
  onOpenPlan,
  onReference,
}: {
  events: EventLine[];
  onOpenTask: (task: number) => void;
  onOpenPlan?: (plan: number, version: number) => void;
  onReference?: (ref: string) => void;
}) {
  return (
    <MessagePrimitive.Root className="events">
      {events.map((e, index) => (
        // Two lines can read the same ("You answered Orin: Done"): its task tells them apart, else its place.
        <Event
          key={e.task ?? `at-${index}`}
          event={e}
          onOpenTask={onOpenTask}
          onOpenPlan={onOpenPlan}
          onReference={onReference}
        />
      ))}
    </MessagePrimitive.Root>
  );
}

/**
 * One line, cut with … when long; a click shows it whole, with what more there is (ux-issues 27) and
 * its task, which opens in the Tasks tab (room task 70), or an ask's sizes (0061).
 */
function Event({
  event,
  onOpenTask,
  onOpenPlan,
  onReference,
}: {
  event: EventLine;
  onOpenTask: (task: number) => void;
  onOpenPlan?: (plan: number, version: number) => void;
  onReference?: (ref: string) => void;
}) {
  const { who, text, more, task, ask, plan } = event;
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
      {(more || task !== undefined || ask || plan) && (
        <CollapsibleContent className="event-more">
          {more && <span>{more}</span>}
          {task !== undefined && (
            <button type="button" className="link" onClick={() => onOpenTask(task)}>
              Open task
            </button>
          )}
          {plan && onOpenPlan && (
            <button type="button" className="link" onClick={() => onOpenPlan(plan.id, plan.version)}>
              Open plan
            </button>
          )}
          {ask && (
            <SizeChoice ask={ask} size={event.kind === 'fix' ? 'repair' : 'new'} onReference={onReference} />
          )}
        </CollapsibleContent>
      )}
    </CollapsibleRoot>
  );
}
