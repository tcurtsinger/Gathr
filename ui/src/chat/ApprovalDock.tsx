import { Button } from '@/components/ui/button';
import type { PlanSummary } from '../api';
import { Face, type FaceOf } from '../components/Face';
import { counted } from '../plans/plans';
import { useMarks } from './faceMarks';
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
  /** The chat's plans: one waiting on the person waits here after the approvals (0062). */
  plans: PlanSummary[];
  /** The agent whose turn asked. */
  asker: (turn: string) => FaceOf;
  /** A plan's lead, by id. */
  lead: (agent: string) => FaceOf;
  /** Scrolls to the approval where it was asked. */
  onShow: (approval: Approval) => void;
  onAnswer: (request: string, allow: boolean) => void;
  /** Opens the plan's tab at its newest version. */
  onOpenPlan: (plan: number, version: number) => void;
}

/**
 * What waits on the person, above the composer wherever the chat is scrolled to (0037): the oldest approval,
 * else the oldest plan waiting, and how many wait. An approval being sent has left it; a refused answer
 * brings it back.
 */
export function ApprovalDock({ approvals, plans, asker, lead, onShow, onAnswer, onOpenPlan }: Props) {
  const marks = useMarks();
  const waiting = approvals.filter(unanswered);
  const planned = plans.filter((p) => p.state === 'waiting');
  const count = waiting.length + planned.length;
  const a = waiting[0];
  if (!a) {
    const plan = planned[0];
    const newest = plan?.versions.at(-1);
    if (!plan || !newest) return null;
    const agent = lead(plan.lead);
    return (
      <section className="needs-dock" aria-label="Needs you">
        <Face agent={agent} size={24} mark={marks?.still.get(agent.id)} />
        <div className="needs-text">
          <small>Needs you · 1 of {count}</small>
          <div>
            {agent.name}'s plan: {plan.title} · {counted(newest.parts, 'part')}
          </div>
        </div>
        <Button type="button" onClick={() => onOpenPlan(plan.id, newest.n)}>
          Open
        </Button>
      </section>
    );
  }
  const agent = asker(a.turn);
  const { tool, command } = asked(a);
  return (
    <section className="needs-dock" aria-label="Needs you">
      <Face agent={agent} size={24} mark={marks?.still.get(agent.id)} />
      <div className="needs-text">
        <small>Needs you · 1 of {count}</small>
        <div>
          {agent.name} wants to run <code>{command ?? tool}</code>
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
