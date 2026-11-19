import { MessagePrimitive, useAuiState } from '@assistant-ui/react';
import { useContext, useEffect, useRef, useState } from 'react';
import { ApprovalCard } from '@/components/assistant-ui/elements/approval-card';
import { Button } from '@/components/ui/button';
import type { Failure } from '../api';
import { MarkdownText } from '../components/Markdown';
import { FailureNotice } from '../components/Notice';
import { Outcome } from '../components/Outcome';
import { PickedText } from '../components/PickedText';
import { Asked } from './Asked';
import type { Ending } from './ends';
import { Foot } from './Foot';
import type { Like } from './likes';
import { MainWord } from './MainWord';
import type { Custom } from './messages';
import {
  AgentFace,
  Bubble,
  EndRow,
  EndWord,
  NameLine,
  NewWord,
  OnTask,
  RoleWord,
  Said,
  When,
  YouSaid,
} from './Said';
import { SizeWord } from './Size';
import { StepsButton, useStepsOpen } from './Steps';
import type { Approval } from './state';
import { Earlier, TalkMessage } from './Talk';
import { TaskMain } from './TaskMain';
import { type Actions, useActions, useJump } from './Thread';
import { ThreadBox } from './ThreadBox';
import { FoundContext, PlanChip, WorkRow } from './TurnParts';

export const custom = (s: { message: { metadata: { custom: Record<string, unknown> } } }) =>
  s.message.metadata.custom as Custom;

/**
 * The person's own words, beside their face, in the blue-tinted bubble (0072), with their ask's size last on the name
 * line (0061); one not sent says so, with Retry.
 */
export function UserMessage() {
  const own = useAuiState(custom);
  const { unsent, size } = own;
  const actions = useActions();
  const [refused, setRefused] = useState<Failure>();
  // A failed Stop or Pause sent no text: only its notice shows, with nothing to resend.
  const stop = unsent?.text === '';
  const failure = unsent?.failure;
  return (
    <Said
      root
      you
      reply={own.reply}
      small={own.reply}
      bare={stop}
      face={<YouSaid small={own.reply} />}
      line={
        <NameLine
          name="You"
          pieces={[
            <When key="t" time={own.tick ?? own.time} />,
            unsent && !stop && <EndWord key="e" word="Not sent" bad />,
            size && <SizeWord key="size" size={size} onFailure={setRefused} />,
          ]}
          state={<MainWord state={own.state} />}
        />
      }
    >
      {!stop && (
        // Agents' thumbs-ups, never the person's own like: no `like`, so no Like and no toggle (0057 C5).
        <Bubble foot={<Foot fold={own.thread} agreed={own.agreed} answer={own.answer} />}>
          <div className="bubble-plain">
            <MessagePrimitive.Parts components={personParts} />
          </div>
        </Bubble>
      )}
      {refused && <FailureNotice failure={refused} onReference={actions.reference} />}
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

function PersonText({ text }: { text: string }) {
  return <PickedText text={text} picks={useAuiState(custom).picks} />;
}
const personParts = { Text: PersonText };

/** An agent's message, or what a thread draws between them: its fold line, "Show earlier", a task handed out. */
export function AssistantMessage() {
  const own = useAuiState(custom);
  const id = useAuiState((s) => s.message.id);
  if (own.box) return <ThreadBox topic={own.box.topic} />;
  if (own.newLine) return <NewLine />;
  if (own.earlier) return <Earlier earlier={own.earlier} />;
  if (own.brief) return <TaskMain own={own} />;
  if (own.said) return <TalkMessage own={own} said={own.said} />;
  return <Reply own={own} id={id} />;
}

/** Above the first main message said while the person was away (0077). */
function NewLine() {
  return (
    <MessagePrimitive.Root className="new-line" role="separator">
      New
    </MessagePrimitive.Root>
  );
}

/**
 * A block of an agent's turn: its face and name line (with how its task or turn ended, and its steps), its words in a
 * bubble with its plan's chip, thumbs-ups, then its approvals and what the person can do about how it ended. A block
 * with nothing to show draws nothing: a turn that has just started shows as its agent's Working mark, not as an empty message.
 * A main message says its state at its name line's right; an ask's result quotes the ask (0075). A reply sits in its
 * thread, smaller.
 */
export function Reply({ own, id }: { own: Custom; id: string }) {
  const { agent, by, role, on, done, out, work, ending, approvals, agreed, plan, asked, result } = own;
  const actions = useActions();
  const jump = useJump();
  const [open, setOpen] = useStepsOpen(work);
  const small = own.reply;
  const words = !!own.text || !!plan || !!asked?.length;
  if (!words && !work && !approvals?.length && !ending && !out && !agreed?.length) return null;
  // How it ended says who: a block of its own is named.
  const named = !!agent && (own.head || !!work || !!ending?.word || !!out || !!done);
  // Steps, and how it ended, need their line even before the agent's name is known.
  const lined = named || !!work || !!ending?.word;
  return (
    <Said
      root
      small={small}
      reply={small}
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
            size={small ? 28 : 36}
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
              // In a thread the lead is named alone, as every reply is.
              role === 'supervisor' && !small && <RoleWord key="role" />,
              <When key="t" time={own.time} />,
              small && <NewWord key="new" topic={own.topic} time={own.time} />,
              // The smallest honest mark of an ask's result: it closes the ask it quotes.
              result && 'Result',
              on && <OnTask key="on" what={on} done={done} />,
              out && <Outcome key="out" out={out} />,
              ending?.word && <EndWord key="end" word={ending.word} bad={ending.bad} />,
            ]}
            end={work && <StepsButton work={work} open={open} onOpen={setOpen} />}
            state={<MainWord state={own.state} />}
          />
        )
      }
    >
      {/* Closed, a finished turn's steps aren't drawn at all: a long chat mounts no list it doesn't show. */}
      {work && (open || work.running) && (
        <WorkRow work={work} onOpenTask={actions.openTask} open={open} onOpenChange={setOpen} />
      )}
      {words && (
        <Bubble
          quote={result?.quote}
          onQuote={result && (() => jump(result.ask))}
          foot={<Foot fold={own.thread} agreed={agreed} like={own.like} answer={own.answer} />}
        >
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
      {approvals?.map((a) => (
        <AskApproval key={a.request} approval={a} agent={agent ?? ''} actions={actions} />
      ))}
      {ending?.act && <EndActs ending={ending} agent={by} actions={actions} />}
    </Said>
  );
}

/** The person's like on a message, as its corner row and its count press it (0057). */
export function liking(like: Like | undefined, actions: Actions) {
  return (
    like && { given: like.mine, onToggle: () => actions.like?.(like.message, like.questions, !like.mine) }
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

/** What an approval asks to run: the tool as the person knows it, and its command when it has one. */
function asked(a: Approval): { tool: string; command?: string } {
  const input = a.input as { command?: unknown; check?: unknown } | null | undefined;
  // Claude's hook asks for a wait check under the tool's name, with the command as `check`.
  const claudeCheck = a.tool === 'mcp__gathr__wait' ? input?.check : undefined;
  const tool = typeof claudeCheck === 'string' ? 'wait check' : a.tool;
  const command = tool === a.tool ? input?.command : claudeCheck;
  return { tool, command: typeof command === 'string' ? command : undefined };
}

/** Under the bubble, what the person can do about how the turn ended: Continue, Retry with its reference, or Sign in. */
function EndActs({ ending, agent, actions }: { ending: Ending; agent?: string; actions: Actions }) {
  const { turn, act } = ending;
  if (act === 'reference')
    return <EndRow reference={turn.ref} onReference={() => actions.diagnostics(turn.id)} />;
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
