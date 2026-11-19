import {
  Bell,
  CheckCheck,
  ChevronRight,
  CircleCheck,
  CircleCheckBig,
  CircleHelp,
  ClipboardList,
  FilePen,
  PenLine,
  Send,
} from 'lucide-react';
import { createContext, useState } from 'react';
import {
  CollapsibleContent,
  CollapsibleRoot,
  CollapsibleTrigger,
} from '@/components/assistant-ui/elements/collapsible-root';
import { RunActivity, type RunActivityEntry } from '@/components/assistant-ui/elements/run-activity';
import { ToolCall } from '@/components/assistant-ui/elements/tool-call';
import type { ReviewNote } from '../api';
import { counted } from '../plans/plans';
import { Note } from '../tasks/Note';
import type { Step, Work } from './fold';
import type { EventLine } from './messages';
import { LiveTime } from './Steps';
import './team.css';
import '../plans/plans.css';

/** The text a search hit opened at. */
export const FoundContext = createContext<string | undefined>(undefined);

/**
 * Steps as one row, assistant-ui's Run activity (the owner's calls, 2026-11-12 and 2026-11-19): in a bubble, a run of
 * steps between words, closed to what it did ("Read 2 files, ran 1 command"); in the agent's panel's Now, a running
 * turn's steps. While it runs it says "Working", shimmering, its latest step under it; open, each tool step is a Tool
 * call, which opens to what it ran and what came back (0044), and Gathr's own lines sit between them: its thoughts,
 * the tasks it gave, and in the panel what it wrote mid-work.
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
  const [open, setOpen] = useState(startOpen);
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
    content: <StepEntry step={step} onOpenTask={onOpenTask} />,
  }));
  return (
    <RunActivity
      // Closed, only "Working" and the latest step show: those shimmer, never a list of steps opened.
      className={work.running && !open ? 'work shimmer motion-reduce:animate-none' : 'work'}
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
function StepEntry({ step, onOpenTask }: { step: Step; onOpenTask: (task: number) => void }) {
  const [open, setOpen] = useState(false);
  if ('say' in step) return <p className="work-say">{step.say}</p>;
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

/**
 * A plan check's findings at the end of its checker's bubble (0078), in the plan chip's shape: "7 findings" (wrong and
 * not asked for), or "2 suggestions" for a report of suggestions only. A press opens it to each note, in order.
 */
export function FindingsChip({ notes }: { notes: ReviewNote[] }) {
  const found = notes.filter((n) => n.kind !== 'suggestion').length;
  return (
    <CollapsibleRoot className="findings">
      <CollapsibleTrigger className="plan-chip findings-chip">
        <CircleCheckBig aria-hidden="true" />
        <span>{found ? counted(found, 'finding') : counted(notes.length, 'suggestion')}</span>
        <ChevronRight className="findings-open" aria-hidden="true" />
      </CollapsibleTrigger>
      <CollapsibleContent asChild>
        <ul className="findings-notes">
          {notes.map((note, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: two notes may read the same
            <li key={i}>
              <Note note={note} />
            </li>
          ))}
        </ul>
      </CollapsibleContent>
    </CollapsibleRoot>
  );
}

/** A member's check of an answer (0080), first in its bubble, as SerenityOS draws it: "Checked Mira's answer". */
export function CheckedLine({ author }: { author: string }) {
  return (
    <div className="checked-line">
      <CheckCheck aria-hidden="true" />
      <span>Checked {author}'s answer</span>
    </div>
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
