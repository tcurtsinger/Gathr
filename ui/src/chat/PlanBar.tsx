import { ChevronUpIcon, CircleCheckIcon, LoaderIcon } from 'lucide-react';
import { useMemo } from 'react';
import { AgentPlan } from '@/components/assistant-ui/elements/agent-plan';
import {
  CollapsibleContent,
  CollapsibleRoot,
  CollapsibleTrigger,
} from '@/components/assistant-ui/elements/collapsible-root';
import { currentRound } from './plan';
import type { ChatState } from './state';
import './plan.css';

/** Gathr has no plan records yet: until it does, the bar is titled Plan (never a title made from text). */
const TITLE = 'Plan';

interface Props {
  state: ChatState;
  /** Kept per chat by the app. */
  open: boolean;
  setOpen: (open: boolean) => void;
}

/**
 * The Supervisor's current round, docked above the composer (DESIGN.md "Agent plan"): its state, the
 * segments of its steps and done/total; opening it lists the steps above the row.
 */
export function PlanBar({ state, open, setOpen }: Props) {
  const round = useMemo(() => currentRound(state), [state]);
  if (!round) return null;
  const { steps, done } = round;
  const label = `${TITLE}${round.status ? `: ${round.status}` : ''}. ${done} of ${steps.length} done`;
  return (
    <CollapsibleRoot className="plan-bar" open={open} onOpenChange={setOpen}>
      <CollapsibleContent className="plan-steps">
        <AgentPlan
          title={TITLE}
          header={false}
          activeIndex={done}
          className="max-w-none"
          steps={steps.map((s) => ({
            id: String(s.task),
            label: s.label,
            status: s.state,
            owner: s.owner,
            description: s.detail,
          }))}
        />
      </CollapsibleContent>
      <CollapsibleTrigger className="plan-row" aria-label={label}>
        <span className="plan-icon" data-done={round.finished || undefined}>
          {round.finished ? (
            <CircleCheckIcon aria-hidden="true" />
          ) : round.busy ? (
            <LoaderIcon
              aria-hidden="true"
              className="animate-spin [animation-duration:1.2s] motion-reduce:animate-none"
            />
          ) : (
            <i aria-hidden="true" />
          )}
        </span>
        <span className="plan-title">
          <strong>{TITLE}</strong>
          <span>{round.status}</span>
        </span>
        <span
          className="plan-segments"
          role="progressbar"
          aria-label={`${TITLE} progress`}
          aria-valuemin={0}
          aria-valuemax={steps.length}
          aria-valuenow={done}
        >
          {steps.map((s) => (
            <i key={s.task} data-state={s.state} />
          ))}
        </span>
        <span className="plan-fraction">
          {done}/{steps.length}
        </span>
        <ChevronUpIcon aria-hidden="true" className="plan-chevron" />
      </CollapsibleTrigger>
    </CollapsibleRoot>
  );
}
