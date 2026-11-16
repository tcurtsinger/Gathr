import type { Message, PlanSummary, PlanVersionSummary } from '../api';
import type { EventLine } from '../chat/messages';
import type { ChatState } from '../chat/state';
import type { Team } from '../chat/team';

/** The version whose message this is, with its plan. */
export function versionOf(plans: PlanSummary[], message: string) {
  for (const plan of plans) {
    const version = plan.versions.find((v) => v.message === message);
    if (version) return { plan, version };
  }
  return undefined;
}

/** "1 part", "9 parts". */
export const counted = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

/**
 * Gathr's line for a plan (0062 H5.2), found by the message that says it, never by its words: a version's
 * check line, or the person's word on it (Approve's and Drop's lines, the send-back's turn).
 */
export function planLine(m: Message, state: ChatState, crew: Team): EventLine | undefined {
  for (const plan of state.plans) {
    for (const v of plan.versions) {
      const line = (text: string, more: string): EventLine => ({
        kind: 'version',
        who: `Plan v${v.n}:`,
        text,
        more,
        plan: { id: plan.id, version: v.n },
      });
      if (v.check_line === m.id) return line(checked(v, m, state, crew), plan.title);
      if (v.decided_line !== m.id) continue;
      // The send-back's message holds the person's words between its first line and its last.
      if (v.decided === 'sent_back')
        return line('Sent back', (m.body as string).split('\n').slice(1, -1).join('\n'));
      return line(v.decided === 'approved' ? 'Approved' : 'Dropped', plan.title);
    }
  }
  return undefined;
}

/**
 * How the check ended when its line was said: a version that didn't reach the person has findings; one that
 * did was agreed only by a report made by then, as a check that ran late can agree after Gathr sent it on.
 */
function checked(v: PlanVersionSummary, line: Message, state: ChatState, crew: Team): string {
  const check = state.tasks.findLast((t) => t.checks === v.id);
  const by = crew.name(check?.owner) ?? 'An agent';
  if (!v.sent) return counted(check?.notes?.filter((n) => n.kind !== 'suggestion').length ?? 0, 'finding');
  const at = check?.report?.at_us;
  const agreed = check?.state === 'done' && (at == null || at <= (Date.parse(line.time) + 1) * 1000);
  return agreed ? `${by} agrees` : `${by} didn't finish`;
}
