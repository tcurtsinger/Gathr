import { MessagePrimitive, useAuiState } from '@assistant-ui/react';
import { useContext, useEffect, useRef, useState } from 'react';
import { ApprovalCard } from '@/components/assistant-ui/elements/approval-card';
import { Button } from '@/components/ui/button';
import type { Failure } from '../api';
import { MarkdownText } from '../components/Markdown';
import { FailureNotice } from '../components/Notice';
import { PickedText } from '../components/PickedText';
import { Asked } from './Asked';
import type { Ending } from './ends';
import { Foot } from './Foot';
import type { Piece } from './fold';
import type { Like } from './likes';
import type { Custom } from './messages';
import { AgentFace, Bubble, EndRow, NameLine, NewWord, OnTask, Said, Took, When, YouSaid } from './Said';
import { SizeWord } from './Size';
import { InPanel } from './Steps';
import type { Approval } from './state';
import { Earlier, TalkMessage } from './Talk';
import { TaskMain } from './TaskMain';
import { type Actions, useActions, useJump } from './Thread';
import { ThreadBox } from './ThreadBox';
import { CheckedLine, FindingsChip, FoundContext, PlanChip, WorkRow } from './TurnParts';
import { titleOf } from './team';

export const custom = (s: { message: { metadata: { custom: Record<string, unknown> } } }) =>
  s.message.metadata.custom as Custom;

/**
 * The person's own words, beside their face, in the blue-tinted bubble (0072), with their ask's size last on the name
 * line (0061); one not sent says why under it, with Retry.
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
            size && <SizeWord key="size" size={size} onFailure={setRefused} />,
          ]}
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
 * A block of an agent's turn, as SerenityOS's card reads (0082): its face and name line (the name, its title, the time,
 * New, how long it took), then in one bubble its message and its plan's chip where it wrote the plan (`Pieces`), its
 * questions, a plan check's findings chip, and at the foot one line for its steps and narration; thumbs-ups, then its
 * approvals and what the person can do about how it ended. An ask's result quotes the ask (0075); a turn Gathr's words opened quotes them (0078); a member's check of an
 * answer opens with its Checked line (0080). A reply sits in its thread, smaller. In the agent's panel its name line
 * also says the task it worked on.
 */
export function Reply({ own, id }: { own: Custom; id: string }) {
  const {
    agent,
    by,
    seat,
    on,
    done,
    ending,
    approvals,
    agreed,
    asked,
    result,
    opened,
    pieces,
    line,
    findings,
    checked,
  } = own;
  const actions = useActions();
  const jump = useJump();
  const panel = useContext(InPanel);
  const small = own.reply;
  const words = !!pieces || !!line || !!asked?.length || !!findings;
  if (!words && !approvals?.length && !ending) return null;
  // With no bubble, what it holds says who it is from.
  const named = !!agent && (own.head || !words);
  return (
    <Said
      root
      small={small}
      reply={small}
      agent={by}
      face={
        named && (
          <AgentFace
            agent={by}
            name={agent}
            message={id}
            size={small ? 28 : 36}
            // An agent of the chat has a panel to open; one that left it has none.
            onOpen={by && seat ? () => actions.openAgent(by) : undefined}
          />
        )
      }
      line={
        named && (
          <NameLine
            name={agent ?? ''}
            title={titleOf(own.role)}
            pieces={[
              <When key="t" time={own.time} />,
              small && <NewWord key="new" topic={own.topic} time={own.time} />,
              own.took && <Took key="took" took={own.took} />,
              panel && on && <OnTask key="on" what={on} done={done} />,
            ]}
          />
        )
      }
    >
      {words && (
        <Bubble
          lead={checked && <CheckedLine author={checked} />}
          quote={result?.quote ?? opened}
          onQuote={result && (() => jump(result.ask))}
          quoteLines={result ? undefined : 3}
          foot={<Foot fold={own.thread} agreed={agreed} like={own.like} answer={own.answer} />}
        >
          {pieces && <Pieces pieces={pieces} actions={actions} />}
          {asked?.map((n) => (
            <Asked key={n} task={n} />
          ))}
          {findings && <FindingsChip notes={findings} />}
          {line && <WorkRow work={line} onOpenTask={actions.openTask} />}
        </Bubble>
      )}
      {approvals?.map((a) => (
        <AskApproval key={a.request} approval={a} agent={agent ?? ''} actions={actions} />
      ))}
      {ending && <EndActs ending={ending} agent={by} actions={actions} />}
    </Said>
  );
}

/**
 * A turn's message in its bubble, in the order it came (0082): each text drawn as a message's text is, and a plan
 * version's chip, which opens the plan's tab there (0078).
 */
export function Pieces({ pieces, actions }: { pieces: Piece[]; actions: Actions }) {
  return pieces.map((p) => {
    if ('plan' in p) {
      const { plan, version, parts } = p.plan;
      return (
        <PlanChip
          key={p.id}
          version={version}
          parts={parts}
          onOpen={() => actions.openPlan?.(plan, version)}
        />
      );
    }
    // By its place, not its message: text still streaming keeps its node once saved.
    return <MessagePrimitive.PartByIndex key={`words-${p.words}`} index={p.words} components={parts} />;
  });
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
