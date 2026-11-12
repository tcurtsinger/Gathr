import {
  CircleAlertIcon,
  CircleCheckIcon,
  CircleIcon,
  LoaderIcon,
  Pause,
  Play,
  SquareIcon,
  SquareMinusIcon,
} from 'lucide-react';
import { Popover } from 'radix-ui';
import { useMemo, useRef } from 'react';
import { TooltipIconButton } from '@/components/assistant-ui/elements/tooltip-icon-button';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { PauseScope, Pauses } from '../api';
import { counts, type Glance, headline } from './glance';
import { Initial, InitialStack } from './Initial';
import { agentPause, type PauseAction } from './pauses';
import { currentRound, type Round, type StepState } from './plan';
import type { ChatState } from './state';
import './summary.css';

/** Shown while nobody works or needs the person, so the button is never empty: two agents, each with a line. */
function SummaryIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden="true">
      <circle cx="6" cy="6.5" r="2.5" />
      <path d="M12 6.5h8" strokeLinecap="round" />
      <circle cx="6" cy="17.5" r="2.5" />
      <path d="M12 17.5h8" strokeLinecap="round" />
    </svg>
  );
}

interface Props {
  state: ChatState;
  /** Every agent of the chat, in Summary's order. */
  team: Glance[];
  open: boolean;
  onOpen: (open: boolean) => void;
  /** What last opened or closed it other than its own button (the Working line): closing gives focus back to it. */
  from?: HTMLElement | null;
  /** Its own button was pressed, so focus belongs there again. */
  onOwnButton?: () => void;
  pauses: Pauses;
  onPause: (action: PauseAction, scope: PauseScope) => void;
  /** Opens an agent's panel in the workspace. */
  onOpenAgent: (agent: string) => void;
}

/**
 * The team at a glance (DESIGN.md "Summary", 0038). Its button in the chat header is the overlapped
 * initials of whoever works, each with a turning ring, then whoever needs the person, with one dot on the
 * stack. It opens a popover: the Supervisor's current round as Plan, then every agent on one line. A step
 * or an agent opens that agent's panel. Escape or a press outside closes it.
 */
export function Summary(props: Props) {
  const { state, team, open, onOpen, from, onOwnButton, pauses, onPause, onOpenAgent } = props;
  const round = useMemo(() => currentRound(state), [state]);
  const said = headline(team);
  const label = said ? `Summary: ${said}` : 'Summary';
  // Whoever works first, each with its ring; whoever needs the person last, with none.
  const stack = team
    .filter((g) => g.standing === 'working' || g.standing === 'needs')
    .sort((a, b) => Number(a.standing === 'needs') - Number(b.standing === 'needs'))
    .map((g) => ({ agent: g.member.agent, name: g.member.name, busy: g.standing === 'working' }));
  const needs = new Set(team.filter((g) => g.standing === 'needs').map((g) => g.member.agent));
  // Opening an agent moves the keyboard to its panel: closing must not bring it back to the button, whose
  // tooltip would open over the panel.
  const went = useRef(false);
  const go = (agent: string) => {
    went.current = true;
    onOpen(false);
    onOpenAgent(agent);
  };
  return (
    <Popover.Root open={open} onOpenChange={onOpen}>
      <Popover.Trigger asChild>
        <TooltipIconButton tooltip={label} className="head-button summary-button" onClick={onOwnButton}>
          {stack.length ? <InitialStack agents={stack} needs={needs.size > 0} /> : <SummaryIcon />}
        </TooltipIconButton>
      </Popover.Trigger>
      <Popover.Anchor asChild>
        <span className="summary-anchor" aria-hidden="true" />
      </Popover.Anchor>
      <Popover.Portal>
        <Popover.Content
          className="summary"
          side="bottom"
          align="end"
          sideOffset={8}
          collisionPadding={16}
          role="dialog"
          aria-label="Summary"
          onInteractOutside={(e) => {
            if ((e.target as Element).closest?.('[data-summary-opener]')) e.preventDefault();
          }}
          onCloseAutoFocus={(e) => {
            if (went.current) e.preventDefault();
            else if (from?.isConnected) {
              e.preventDefault();
              from.focus();
            }
            went.current = false;
          }}
        >
          {round && (
            <Plan
              round={round}
              needs={needs}
              // An agent that left the chat still has its step, but no panel to open.
              opener={(agent) => (team.some((g) => g.member.agent === agent) ? () => go(agent) : undefined)}
            />
          )}
          <h3>
            Team<small className="summary-note">{counts(team)}</small>
          </h3>
          <div className="summary-team">
            {team.map((g) => (
              <button
                type="button"
                className="agent-row"
                key={g.member.agent}
                onClick={() => go(g.member.agent)}
              >
                <Initial name={g.member.name} busy={g.standing === 'working'} />
                <span className="agent-name">{g.member.name}</span>
                <span className="agent-doing" data-standing={g.standing}>
                  {g.doing}
                </span>
                <span className="agent-count">
                  {g.total > 0 && (
                    // "1/2" would be read out as a half.
                    <>
                      <span aria-hidden="true">
                        {g.done}/{g.total}
                      </span>
                      <span className="visually-hidden">
                        {g.done} of {g.total} {g.total === 1 ? 'task' : 'tasks'} done
                      </span>
                    </>
                  )}
                </span>
              </button>
            ))}
          </div>
          <footer className="summary-foot">
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button type="button" variant="ghost" size="xs">
                  Pause or resume
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start" sideOffset={4}>
                {team.map(({ member }) => {
                  const paused = agentPause(pauses, member.agent).paused;
                  const scope: PauseScope = { scope: 'agent', target: member.agent };
                  return (
                    <DropdownMenuItem
                      key={member.agent}
                      onSelect={() => onPause(paused ? 'resume' : 'pause', scope)}
                    >
                      {paused ? <Play /> : <Pause />}
                      {paused ? 'Resume' : 'Pause'} {member.name}
                    </DropdownMenuItem>
                  );
                })}
              </DropdownMenuContent>
            </DropdownMenu>
          </footer>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

const STEP: Record<StepState, { icon: typeof CircleIcon; spoken: string; mark?: string }> = {
  done: { icon: CircleCheckIcon, spoken: 'done' },
  active: { icon: LoaderIcon, spoken: 'in progress' },
  pending: { icon: CircleIcon, spoken: 'not started' },
  blocked: { icon: SquareMinusIcon, spoken: 'blocked', mark: 'blocked' },
  failed: { icon: CircleAlertIcon, spoken: 'failed', mark: 'failed' },
  stopped: { icon: SquareIcon, spoken: 'stopped', mark: 'stopped' },
};

/** The round's steps: each task's state, what it is, and who has it. Only done tasks count as done. */
function Plan({
  round,
  needs,
  opener,
}: {
  round: Round;
  needs: Set<string>;
  /** What opens an agent's panel; none for an agent no longer in the chat. */
  opener: (agent: string) => (() => void) | undefined;
}) {
  const { steps, done } = round;
  return (
    <section className="summary-plan" aria-label="Plan">
      <h3>
        Plan
        <small className="summary-note">
          <span
            className="plan-segments"
            role="progressbar"
            aria-label="Plan progress"
            aria-valuemin={0}
            aria-valuemax={steps.length}
            aria-valuenow={done}
          >
            {steps.map((s) => (
              <i key={s.task} data-state={s.state} />
            ))}
          </span>
          {done} of {steps.length} done
        </small>
      </h3>
      {steps.map((s) => {
        const { icon: Icon, spoken, mark } = STEP[s.state];
        // The person holds up a step only while it is at work.
        const waits = s.state === 'active' && needs.has(s.agent);
        const open = opener(s.agent);
        const row = (
          <>
            <Icon
              aria-hidden="true"
              className={s.state === 'active' ? 'animate-spin motion-reduce:animate-none' : undefined}
            />
            <span className="plan-step-text">{s.label}</span>
            <span className="visually-hidden">, {spoken}, </span>
            <small data-needs={waits || undefined}>
              {s.owner}
              {waits ? ' · needs you' : mark && ` · ${mark}`}
            </small>
          </>
        );
        return open ? (
          <button type="button" className="plan-step" key={s.task} data-state={s.state} onClick={open}>
            {row}
          </button>
        ) : (
          <div className="plan-step" key={s.task} data-state={s.state}>
            {row}
          </div>
        );
      })}
    </section>
  );
}
