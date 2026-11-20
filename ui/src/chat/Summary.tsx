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
import { useEffect, useMemo, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { PauseScope, Pauses, Waiting } from '../api';
import { Face, GathrFace } from '../components/Face';
import { IconButton } from '../components/IconButton';
import { CANT_READ, cardLine, since } from '../components/marks';
import { useMarks } from './faceMarks';
import { counts, type Glance, headline } from './glance';
import { agentPause, type PauseAction } from './pauses';
import { currentRound, type Round, type StepState } from './plan';
import type { ChatState } from './state';
import { inWords } from './statusWords';
import './summary.css';

interface Props {
  state: ChatState;
  /** Every agent of the chat, in Summary's order. */
  team: Glance[];
  open: boolean;
  onOpen: (open: boolean) => void;
  pauses: Pauses;
  onPause: (action: PauseAction, scope: PauseScope) => void;
  /** Opens an agent's panel in the workspace. */
  onOpenAgent: (agent: string) => void;
  /** The service's answer: Gathr's own waits, and which running turn is silent since when (0050, 0052). */
  waiting?: Waiting;
  /** Goes to what the person is needed for, if the chat holds it: false when it holds nothing to go to. */
  onNeeds?: () => boolean;
}

/**
 * The team at a glance (DESIGN.md "Summary", 0038). Its button in the top line is every agent of the chat in the
 * chat's order, apart, each with its state's mark (0071). It opens a popover: the Supervisor's current round as
 * Plan, then every agent on one line, and Gathr's own waits after them. A step or an agent opens that agent's panel;
 * an agent that needs the person goes to what it needs them for. Escape or a press outside closes it.
 */
export function Summary(props: Props) {
  const { state, team, open, onOpen, pauses, onPause, onOpenAgent, waiting, onNeeds } = props;
  const round = useMemo(() => currentRound(state), [state]);
  const said = headline(team);
  const label = said ? `Summary: ${said}` : 'Summary';
  const marks = useMarks();
  const shown = (agent: string) => (marks?.archived ? undefined : marks?.faces.get(agent));
  const needs = new Set(team.filter((g) => g.standing === 'needs').map((g) => g.member.agent));
  // Opening an agent moves the keyboard to its panel: closing must not bring it back to the button.
  const went = useRef(false);
  const go = (agent: string) => {
    went.current = true;
    onOpen(false);
    onOpenAgent(agent);
  };
  const press = (g: Glance) => {
    if (g.standing !== 'needs' || !onNeeds?.()) return go(g.member.agent);
    went.current = true;
    onOpen(false);
  };
  return (
    <Popover.Root open={open} onOpenChange={onOpen}>
      <Popover.Trigger asChild>
        <IconButton aria-label={label} className="head-button summary-button">
          <span className="roster">
            {state.members.map((m) => (
              <Face key={m.agent} agent={{ id: m.agent, name: m.name }} size={20} mark={shown(m.agent)} />
            ))}
          </span>
        </IconButton>
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
          onCloseAutoFocus={(e) => {
            if (went.current) e.preventDefault();
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
          <Team team={team} waiting={waiting} archived={!!marks?.archived} onPress={press} />
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

/**
 * One row for each agent: its face and mark, its name, what it is doing, and its tasks done; a silent turn says for how
 * long it has made no progress. Then one for each of Gathr's own waits, with Gathr's face, in its card's words; they
 * open nothing. Counted each second.
 */
function Team({
  team,
  waiting,
  archived,
  onPress,
}: {
  team: Glance[];
  waiting?: Waiting;
  archived: boolean;
  onPress: (g: Glance) => void;
}) {
  const silent = new Map(
    (waiting?.lines ?? []).flatMap((l) => (l.agent && l.silent_us ? [[l.agent, l.silent_us / 1000]] : [])),
  );
  const own = waiting?.gathr ?? [];
  const [now, setNow] = useState(Date.now);
  const ticks = silent.size > 0 || own.length > 0;
  useEffect(() => {
    if (!ticks) return;
    const tick = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(tick);
  }, [ticks]);
  return (
    <div className="summary-team">
      {team.map((g) => {
        const quiet = silent.get(g.member.agent);
        return (
          <button type="button" className="agent-row" key={g.member.agent} onClick={() => onPress(g)}>
            <Face
              agent={{ id: g.member.agent, name: g.member.name }}
              size={24}
              mark={archived ? undefined : { state: g.standing }}
            />
            <span className="agent-name">{g.member.name}</span>
            <span className="agent-doing" data-standing={g.standing}>
              {quiet === undefined || g.doing.includes('silent')
                ? g.doing
                : `${g.doing} · silent ${since(now - quiet)}`}
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
        );
      })}
      {own.map((o) => {
        const what = o.name ? `${o.name}: ${o.what}` : inWords(o.what);
        // The time at the row's end, where an agent's tasks are, so a long what is cut and never the time.
        const began = o.since_us === null ? undefined : o.since_us / 1000;
        return (
          <div className="agent-row" key={`${o.kind}:${o.name}:${o.what}`}>
            <GathrFace size={24} />
            <span className="agent-name">Gathr</span>
            <span className="agent-doing" data-standing={o.state}>
              {cardLine(o.state, [what, o.failing ? CANT_READ : undefined])}
            </span>
            <span className="agent-count">
              {began !== undefined && since(now - began, o.state === 'working')}
            </span>
          </div>
        );
      })}
    </div>
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
    <section className="summary-plan" aria-label="Round">
      <h3>
        Round
        <small className="summary-note">
          <span
            className="plan-segments"
            role="progressbar"
            aria-label="Round progress"
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
