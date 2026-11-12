import { Button } from '@/components/ui/button';
import { Initial } from './Initial';
import { type Approval, unanswered } from './state';

/** What an approval asks to run: the tool as the person knows it, and its command when it has one. */
export function asked(a: Approval): { tool: string; command?: string } {
  const input = a.input as { command?: unknown; check?: unknown } | null | undefined;
  // Claude's hook asks for a wait check under the tool's name, with the command as `check`.
  const claudeCheck = a.tool === 'mcp__gathr__wait' ? input?.check : undefined;
  const tool = typeof claudeCheck === 'string' ? 'wait check' : a.tool;
  const command = tool === a.tool ? input?.command : claudeCheck;
  return { tool, command: typeof command === 'string' ? command : undefined };
}

interface Props {
  approvals: Approval[];
  /** The agent whose turn asked. */
  name: (turn: string) => string;
  /** Scrolls to the approval where it was asked. */
  onShow: (approval: Approval) => void;
  onAnswer: (request: string, allow: boolean) => void;
}

/**
 * The approvals waiting on the person, above the composer wherever the chat is scrolled to (0037): the
 * oldest, and how many wait. One being sent has left it; a refused answer brings it back.
 */
export function ApprovalDock({ approvals, name, onShow, onAnswer }: Props) {
  const waiting = approvals.filter(unanswered);
  const a = waiting[0];
  if (!a) return null;
  const agent = name(a.turn);
  const { tool, command } = asked(a);
  return (
    <section className="needs-dock" aria-label="Needs you">
      <Initial name={agent} />
      <div className="needs-text">
        <small>Needs you · 1 of {waiting.length}</small>
        <div>
          {agent} wants to run <code>{command ?? tool}</code>
        </div>
      </div>
      <Button type="button" variant="ghost" onClick={() => onShow(a)}>
        Show
      </Button>
      <Button type="button" variant="outline" onClick={() => onAnswer(a.request, false)}>
        Deny
      </Button>
      <Button type="button" onClick={() => onAnswer(a.request, true)}>
        Allow
      </Button>
    </section>
  );
}
