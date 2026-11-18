import { MessagePrimitive, useAuiState } from '@assistant-ui/react';
import { useContext, useEffect, useRef } from 'react';
import { ApprovalCard } from '@/components/assistant-ui/elements/approval-card';
import { Button } from '@/components/ui/button';
import { MarkdownText } from '../components/Markdown';
import { FailureNotice, Notice } from '../components/Notice';
import { asked } from './ApprovalDock';
import type { Custom, TurnNotice } from './messages';
import { AgentFace, Bubble, NameLine, OnTask, Reactions, Said, When, YouSaid } from './Said';
import { StepsButton, useStepsOpen } from './Steps';
import type { Approval } from './state';
import { Talk } from './Talk';
import { type Actions, useActions } from './Thread';
import { FoundContext, PlanChip, WorkRow } from './TurnParts';
import { roleName } from './team';

export const custom = (s: { message: { metadata: { custom: Record<string, unknown> } } }) =>
  s.message.metadata.custom as Custom;

/** The person's own words, beside their face, in the blue-tinted bubble (0072). */
export function UserMessage() {
  const own = useAuiState(custom);
  const { unsent } = own;
  const actions = useActions();
  return (
    <Said
      root
      you
      bare={unsent?.text === ''}
      face={<YouSaid />}
      line={<NameLine name="You" pieces={[<When key="t" time={own.tick} />]} />}
    >
      {/* A failed Stop or Pause sent no text: only its notice shows, with nothing to resend. */}
      {unsent?.text !== '' && (
        <Bubble>
          <div className="bubble-plain">
            <MessagePrimitive.Parts />
          </div>
        </Bubble>
      )}
      {unsent && (
        <FailureNotice
          failure={unsent.failure}
          onReference={actions.reference}
          links={<DiagnosticsLink open={() => actions.diagnostics()} />}
          action={
            unsent.text && (
              <Button type="button" variant="outline" onClick={() => actions.resend(unsent)}>
                Retry
              </Button>
            )
          }
        />
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
 * A block of an agent's turn: its face and name line (with its steps), its words in a bubble with its plan's chip,
 * thumbs-ups, then its approvals and how it ended. A block with nothing to show draws nothing: a turn that has
 * just started shows on the status line, not as an empty message.
 */
function Reply({ own, id, text }: { own: Custom; id: string; text: boolean }) {
  const { agent, by, role, on, done, work, notice, approvals, agreed, plan } = own;
  const actions = useActions();
  const [open, setOpen] = useStepsOpen(work);
  if (!text && !work && !plan && !approvals?.length && !notice && !agreed?.length) return null;
  const named = !!agent && (own.head || !!work);
  // Steps need their line even before the agent's name is known.
  const lined = named || !!work;
  return (
    <Said
      root
      agent={by}
      // With no words, the steps are all the block has: they stay shown.
      keep={!!work && (work.running || !!work.failed || (!text && !plan))}
      // Nothing under the name line: the face sits beside it.
      bare={
        !text &&
        !plan &&
        !notice &&
        !approvals?.length &&
        !agreed?.length &&
        !(work && (open || work.running))
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
      {(text || plan) && (
        <Bubble>
          <MessagePrimitive.Parts components={parts} />
          {plan && (
            <PlanChip
              version={plan.version}
              parts={plan.parts}
              onOpen={() => actions.openPlan?.(plan.plan, plan.version)}
            />
          )}
        </Bubble>
      )}
      {!!agreed?.length && <Reactions names={agreed} />}
      {approvals?.map((a) => (
        <AskApproval key={a.request} approval={a} agent={agent ?? ''} actions={actions} />
      ))}
      {notice && <ReplyNotice notice={notice} actions={actions} />}
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

function ReplyNotice({ notice, actions }: { notice: TurnNotice; actions: Actions }) {
  const { turn } = notice;
  if (turn.status === 'stopped') {
    // Its carry-on has started: Continue would only ask for the same work again.
    if (notice.carried) return <Notice title="Stopped, then carried on" />;
    return (
      <Notice
        title="Stopped"
        action={
          <Button type="button" variant="outline" onClick={actions.carryOn}>
            Continue
          </Button>
        }
      />
    );
  }
  if (turn.held === 'claude' || turn.held === 'codex') {
    return (
      <Notice
        title={turn.why ?? (turn.held === 'codex' ? 'Codex is downloading' : 'Claude Code is downloading')}
      />
    );
  }
  if (turn.status === 'waiting_sign_in') {
    return (
      <Notice
        title="Waiting for sign-in"
        action={
          <Button type="button" variant="outline" onClick={actions.signIn}>
            Sign in
          </Button>
        }
      />
    );
  }
  return (
    <Notice
      danger
      title={notice.replied ? 'Reply failed' : 'Send failed'}
      reference={{ ref: turn.ref, open: () => actions.diagnostics(turn.id) }}
      links={<DiagnosticsLink open={() => actions.diagnostics(turn.id)} />}
      action={
        <Button type="button" variant="outline" onClick={() => actions.retry(turn.id)}>
          Retry
        </Button>
      }
    />
  );
}

function DiagnosticsLink({ open }: { open: () => void }) {
  return (
    <button type="button" className="link" onClick={open}>
      Diagnostics
    </button>
  );
}
