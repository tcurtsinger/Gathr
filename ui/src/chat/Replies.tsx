import { MessagePrimitive, useAuiState } from '@assistant-ui/react';
import { useContext, useEffect, useRef } from 'react';
import { ApprovalCard } from '@/components/assistant-ui/elements/approval-card';
import { Button } from '@/components/ui/button';
import { MarkdownText } from '../components/Markdown';
import { FailureNotice } from '../components/Notice';
import { Outcome } from '../components/Outcome';
import { asked } from './ApprovalDock';
import { Asked } from './Asked';
import type { Ending } from './ends';
import type { Custom } from './messages';
import { AgentFace, Bubble, EndRow, EndWord, NameLine, OnTask, Reactions, Said, When, YouSaid } from './Said';
import { StepsButton, useStepsOpen } from './Steps';
import type { Approval } from './state';
import { Talk } from './Talk';
import { type Actions, useActions } from './Thread';
import { FoundContext, PlanChip, WorkRow } from './TurnParts';
import { roleName } from './team';

export const custom = (s: { message: { metadata: { custom: Record<string, unknown> } } }) =>
  s.message.metadata.custom as Custom;

/** The person's own words, beside their face, in the blue-tinted bubble (0072); one not sent says so, with Retry. */
export function UserMessage() {
  const own = useAuiState(custom);
  const { unsent } = own;
  const actions = useActions();
  // A failed Stop or Pause sent no text: only its notice shows, with nothing to resend.
  const stop = unsent?.text === '';
  const failure = unsent?.failure;
  return (
    <Said
      root
      you
      bare={stop}
      face={<YouSaid />}
      line={
        <NameLine
          name="You"
          pieces={[
            <When key="t" time={own.tick ?? own.time} />,
            unsent && !stop && <EndWord key="e" word="Not sent" bad />,
          ]}
        />
      }
    >
      {!stop && (
        <Bubble>
          <div className="bubble-plain">
            <MessagePrimitive.Parts />
          </div>
        </Bubble>
      )}
      {unsent && stop && (
        <FailureNotice
          failure={unsent.failure}
          onReference={actions.reference}
          links={<DiagnosticsLink open={() => actions.diagnostics()} />}
        />
      )}
      {unsent && failure && !stop && (
        <EndRow why={failure.why ?? failure.error} reference={failure.ref} onReference={actions.reference}>
          <Button type="button" variant="outline" onClick={() => actions.resend(unsent)}>
            Retry
          </Button>
        </EndRow>
      )}
    </Said>
  );
}

export const parts = { Text: TextPart };

export function AssistantMessage() {
  const own = useAuiState(custom);
  const id = useAuiState((s) => s.message.id);
  if (own.thread) return <Talk said={own.thread} under={!!own.under} />;
  return <Reply own={own} id={id} text={!!own.text} />;
}

/**
 * A block of an agent's turn: its face and name line (with how its task or turn ended, and its steps), its words in a
 * bubble with its plan's chip, thumbs-ups, then its approvals and what the person can do about how it ended. A block
 * with nothing to show draws nothing: a turn that has just started shows on the status line, not as an empty message.
 */
function Reply({ own, id, text }: { own: Custom; id: string; text: boolean }) {
  const { agent, by, role, on, done, out, work, ending, approvals, agreed, plan, asked } = own;
  const actions = useActions();
  const [open, setOpen] = useStepsOpen(work);
  const words = text || !!plan || !!asked?.length;
  if (!words && !work && !approvals?.length && !ending && !out && !agreed?.length) return null;
  // How it ended says who: a block of its own is named.
  const named = !!agent && (own.head || !!work || !!ending?.word || !!out || !!done);
  // Steps, and how it ended, need their line even before the agent's name is known.
  const lined = named || !!work || !!ending?.word;
  return (
    <Said
      root
      agent={by}
      // With no words, the steps are all the block has: they stay shown.
      keep={!!work && (work.running || !!work.failed || !!work.refused || !words)}
      // Nothing under the name line: the face sits beside it.
      bare={
        !words && !ending?.act && !approvals?.length && !agreed?.length && !(work && (open || work.running))
      }
      face={
        named && (
          <AgentFace
            agent={by}
            name={agent}
            message={id}
            size={36}
            // Only a team chat has a panel to open: its agents have roles.
            onOpen={by && role ? () => actions.openAgent(by) : undefined}
          />
        )
      }
      line={
        lined && (
          <NameLine
            name={agent ?? ''}
            pieces={[
              role === 'supervisor' && roleName.supervisor,
              <When key="t" time={own.time} />,
              on && <OnTask key="on" what={on} done={done} />,
              out && <Outcome key="out" out={out} />,
              ending?.word && <EndWord key="end" word={ending.word} bad={ending.bad} />,
            ]}
            end={work && <StepsButton work={work} open={open} onOpen={setOpen} />}
          />
        )
      }
    >
      {/* Closed, a finished turn's steps aren't drawn at all: a long chat mounts no list it doesn't show. */}
      {work && (open || work.running) && (
        <WorkRow work={work} onOpenTask={actions.openTask} open={open} onOpenChange={setOpen} />
      )}
      {words && (
        <Bubble>
          <MessagePrimitive.Parts components={parts} />
          {plan && (
            <PlanChip
              version={plan.version}
              parts={plan.parts}
              onOpen={() => actions.openPlan?.(plan.plan, plan.version)}
            />
          )}
          {asked?.map((n) => (
            <Asked key={n} task={n} />
          ))}
        </Bubble>
      )}
      {!!agreed?.length && <Reactions names={agreed} />}
      {approvals?.map((a) => (
        <AskApproval key={a.request} approval={a} agent={agent ?? ''} actions={actions} />
      ))}
      {ending?.act && <EndActs ending={ending} agent={by} actions={actions} />}
    </Said>
  );
}

/** A reply's text; the one a search hit opened at is wrapped, to centre and tint it alone. */
function TextPart({ text }: { text: string }) {
  return useContext(FoundContext) === text ? (
    <div className="found-part">
      <MarkdownText />
    </div>
  ) : (
    <MarkdownText />
  );
}

/** An approval: sending shows on the card, and a refused answer shows under it, to answer again. */
function AskApproval({
  approval: a,
  agent,
  actions,
}: {
  approval: Approval;
  agent: string;
  actions: Actions;
}) {
  const card = useRef<HTMLDivElement>(null);
  const failure = a.answer?.failure;
  const sending = a.answer && !failure;
  // The answer's button went while it was sent: on a refusal, focus goes back to it.
  useEffect(() => {
    if (!failure || document.activeElement !== document.body) return;
    const slot = a.answer?.allow ? 'approval-card-allow-once' : 'approval-card-deny';
    card.current?.querySelector<HTMLElement>(`[data-slot="${slot}"]`)?.focus();
  }, [failure, a.answer?.allow]);
  const { tool, command } = asked(a);
  return (
    <>
      <ApprovalCard
        ref={card}
        state={sending ? 'running' : 'request'}
        statusLabel={a.answer?.allow ? 'Allowing…' : 'Denying…'}
        title={`Allow ${tool}?`}
        command={command}
        data-approval={a.request}
        description={
          tool === 'wait check' ? 'Runs in sh (Git Bash on Windows), outside the engine sandbox.' : undefined
        }
        style={{ overflowWrap: 'anywhere' }}
        subtitle={agent}
        allowOnceLabel="Allow"
        denyLabel="Deny"
        onAllowOnce={() => actions.approve(a.request, true)}
        onDeny={() => actions.approve(a.request, false)}
      />
      {failure && <FailureNotice failure={failure} onReference={actions.reference} />}
    </>
  );
}

/** Under the bubble, what the person can do about how the turn ended: Continue, Retry with its reference, or Sign in. */
function EndActs({ ending, agent, actions }: { ending: Ending; agent?: string; actions: Actions }) {
  const { turn, act } = ending;
  const [label, onClick] =
    act === 'retry'
      ? ['Retry', () => actions.retry(turn.id)]
      : act === 'sign-in'
        ? ['Sign in', actions.signIn]
        : ['Continue', () => actions.carryOn(agent)];
  return (
    <EndRow reference={act === 'retry' ? turn.ref : null} onReference={() => actions.diagnostics(turn.id)}>
      <Button type="button" variant="outline" onClick={onClick}>
        {label}
      </Button>
    </EndRow>
  );
}

function DiagnosticsLink({ open }: { open: () => void }) {
  return (
    <button type="button" className="link" onClick={open}>
      Diagnostics
    </button>
  );
}
