import { useEffect, useRef } from 'react';
import { ApprovalCard } from '@/components/assistant-ui/elements/approval-card';
import type { Failure } from '../api';
import { useCard } from './Corner';

export interface Approval {
  turn: string;
  request: string;
  tool: string;
  input?: unknown;
  /** Why it is asked, when the service says: Gathr's own asks begin `Gathr asks because`. */
  why?: string | null;
  /** The person's answer, until the service takes it; with its failure if it was refused. */
  answer?: { allow: boolean; failure?: Failure };
}

/** What an approval's buttons do: answer it. */
interface Actions {
  approve: (request: string, allow: boolean) => void;
}

/** An approval: sending shows on the card, and a refused answer is told, to answer again. */
export function AskApproval({
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
  useCard(failure);
  // The answer's button went while it was sent: on a refusal, focus goes back to it.
  useEffect(() => {
    if (!failure || document.activeElement !== document.body) return;
    const slot = a.answer?.allow ? 'approval-card-allow-once' : 'approval-card-deny';
    card.current?.querySelector<HTMLElement>(`[data-slot="${slot}"]`)?.focus();
  }, [failure, a.answer?.allow]);
  const { tool, command } = asked(a);
  return (
    <ApprovalCard
      ref={card}
      state={sending ? 'running' : 'request'}
      statusLabel={a.answer?.allow ? 'Allowing…' : 'Denying…'}
      title={titleOf(tool)}
      command={command}
      data-approval={a.request}
      description={
        tool === 'wait check' ? 'Runs in sh (Git Bash on Windows), outside the sandbox.' : gathrsWhy(a.why)
      }
      style={{ overflowWrap: 'anywhere' }}
      subtitle={agent}
      allowOnceLabel="Allow"
      denyLabel="Deny"
      onAllowOnce={() => actions.approve(a.request, true)}
      onDeny={() => actions.approve(a.request, false)}
    />
  );
}

/** What an engine's own tools ask, in plain words (the owner's pick, 2026-11-23); any other is "Allow <tool>?". */
const TITLES: Record<string, string> = {
  Bash: 'Run a command?',
  Shell: 'Run a command?',
  PowerShell: 'Run a command?',
  Edit: 'Edit a file?',
  MultiEdit: 'Edit a file?',
  NotebookEdit: 'Edit a file?',
  Write: 'Write a file?',
  WebFetch: 'Fetch a page?',
};
const titleOf = (tool: string) => TITLES[tool] ?? `Allow ${tool}?`;

/** Gathr's tools that ask as the agent's Approvals say, in plain words, with what each would add. */
const ADDS: Record<string, [string, 'command' | 'folder']> = {
  mcp__gathr__add_tool: ['adding a tool', 'command'],
  mcp__gathr__add_skill: ['adding a skill', 'folder'],
};

/**
 * Why Gathr itself asks, as a short line: `Writes outside the agent's folders.`, with what Allow covers when the
 * service says. None for an engine's own ask.
 */
function gathrsWhy(why?: string | null) {
  const own = why?.match(/^Gathr asks because (?:it )?(.+)$/)?.[1];
  return own ? `${own[0].toUpperCase()}${own.slice(1)}${own.endsWith('.') ? '' : '.'}` : undefined;
}

/**
 * What an approval asks to run: the tool as the person knows it, and what it would act on: its command, or the file
 * a file tool would write.
 */
function asked(a: Approval): { tool: string; command?: string } {
  const input = a.input as Record<string, unknown> | null | undefined;
  const adds = ADDS[a.tool];
  if (adds) {
    const what = input?.[adds[1]] ?? input?.link;
    return { tool: adds[0], command: typeof what === 'string' ? what : undefined };
  }
  // Claude's hook asks for a wait check under the tool's name, with the command as `check`.
  const claudeCheck = a.tool === 'mcp__gathr__wait' ? input?.check : undefined;
  const tool = typeof claudeCheck === 'string' ? 'wait check' : a.tool;
  const file = input?.file_path ?? input?.notebook_path ?? input?.path;
  const command = tool === a.tool ? (input?.command ?? file) : claudeCheck;
  return { tool, command: typeof command === 'string' ? command : undefined };
}
