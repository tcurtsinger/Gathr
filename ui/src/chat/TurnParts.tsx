import { Bell, CircleCheck, CircleHelp, ClipboardList, FilePen, PenLine, Send } from 'lucide-react';
import { createContext, useContext, useState } from 'react';
import {
  CollapsibleContent,
  CollapsibleRoot,
  CollapsibleTrigger,
} from '@/components/assistant-ui/elements/collapsible-root';
import { RunActivity, type RunActivityEntry } from '@/components/assistant-ui/elements/run-activity';
import { ToolCall } from '@/components/assistant-ui/elements/tool-call';
import { counted } from '../plans/plans';
import type { Step, Work } from './fold';
import type { EventLine } from './messages';
import { LiveTime, useStepsOpen } from './Steps';
import './team.css';
import '../plans/plans.css';

/** The text a search hit opened at. */
export const FoundContext = createContext<string | undefined>(undefined);

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
  open: given,
  onOpenChange,
}: {
  work: Work;
  onOpenTask: (task: number) => void;
  /** Its steps show from the start, as in the agent's panel. */
  startOpen?: boolean;
  /**
   * Opened from elsewhere: a message's name line holds the button (0072), and the row is only its steps, with no
   * button of its own.
   */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}) {
  const found = useContext(FoundContext);
  const [own, setOwn] = useStepsOpen(work, startOpen);
  const open = given ?? own;
  const setOpen = onOpenChange ?? setOwn;
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
      className={given === undefined ? 'work' : 'work steps-only'}
      status={work.running ? 'running' : 'complete'}
      statusLabel={work.running ? 'Working' : work.label}
      durationLabel={
        <span className="work-notes">
          {work.notes.map((note) => `· ${note}`).join(' ')}
          {work.running && work.since !== undefined && <LiveTime since={work.since} before=" · " />}
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
      className={step.refused ? 'work-step refused' : 'work-step'}
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
  version: ClipboardList,
  changed: FilePen,
};

/**
 * One line, cut with … when long; a click shows it whole, with what more there is (ux-issues 27), or its plan, which
 * opens in the plan's tab.
 */
export function Event({
  event,
  onOpenPlan,
}: {
  event: EventLine;
  onOpenPlan?: (plan: number, version: number) => void;
}) {
  const { who, text, more, plan } = event;
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
      {(more || plan) && (
        <CollapsibleContent className="event-more">
          {more && <span>{more}</span>}
          {plan && onOpenPlan && (
            <button type="button" className="link" onClick={() => onOpenPlan(plan.id, plan.version)}>
              Open plan
            </button>
          )}
        </CollapsibleContent>
      )}
    </CollapsibleRoot>
  );
}
