import type { Message, PlanPart, PlanSummary, PlanVersionSummary } from '../api';
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

/** A part's fields the person reads, each compared on its own; `after` names parts and tasks both. */
export type PartField = 'owner' | 'why' | 'done' | 'proof' | 'where' | 'size' | 'after';

export type PartMark =
  | { mark: 'New' }
  | { mark: 'Changed' | 'Unchanged'; was: PlanPart; changed: PartField[] };

/**
 * A version's parts against the version before's (0062 H2.6), matched by trimmed `what`, which the `plan`
 * tool keeps unique: one mark per part, in order, and the parts of the version before that went.
 */
export function marks(parts: PlanPart[], before: PlanPart[]) {
  const what = (list: PlanPart[]) => new Map(list.map((p) => [p.n, p.what.trim()]));
  const [now, then] = [what(parts), what(before)];
  // The parts it comes after by their what, so a move in order isn't a change, then its tasks.
  const after = (p: PlanPart, names: Map<number, string>) => {
    const named = p.after.map((n) => names.get(n)).sort();
    return JSON.stringify([named, [...p.after_tasks].sort()]);
  };
  const marked = parts.map((p): PartMark => {
    const was = before.find((b) => b.what.trim() === p.what.trim());
    if (!was) return { mark: 'New' };
    const fields: PartField[] = ['owner', 'why', 'done', 'proof', 'where'];
    const changed = fields.filter((f) => p[f] !== was[f]);
    if (p.size !== was.size || p.lines !== was.lines) changed.push('size');
    if (after(p, now) !== after(was, then)) changed.push('after');
    return { mark: changed.length ? 'Changed' : 'Unchanged', was, changed };
  });
  const removed = before.filter((b) => !parts.some((p) => p.what.trim() === b.what.trim()));
  return { parts: marked, removed };
}

/** "1 part", "9 parts". */
export const counted = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

/** The newest version's state, in a word. */
export const WORD: Record<PlanSummary['state'], string> = {
  checking: 'Checking',
  waiting: 'Waiting',
  revising: 'Revising',
  approved: 'Approved',
  dropped: 'Dropped',
};

/** Where a version stands, in a word, as the plan's tab and the version's chip say it: an older one is Replaced. */
export const versionWord = (plan: { state: PlanSummary['state']; versions: { n: number }[] }, n: number) =>
  n === Math.max(...plan.versions.map((v) => v.n)) ? WORD[plan.state] : 'Replaced';

/**
 * Gathr's line for a plan (0062 H5.2), found by the message that says it, never by its words: a version's
 * check line, or the person's word on it (Approve's and Drop's lines, the send-back's turn). A check that agrees
 * has no line (null): it is the checker's thumbs-up on the version (0072); nor has one that left findings, which the
 * chip in the checker's bubble says (0078).
 */
export function planLine(m: Message, state: ChatState, crew: Team): EventLine | null | undefined {
  for (const plan of state.plans) {
    for (const v of plan.versions) {
      const line = (text: string, more: string): EventLine => ({
        kind: 'version',
        who: `Plan v${v.n}:`,
        text,
        more,
        plan: { id: plan.id, version: v.n },
      });
      if (v.check_line === m.id) {
        const by = unfinished(v, m, state, crew);
        if (by === null) return null;
        // The version reached the person before its check ended: Gathr says so in a sentence of its own (0073).
        const text = `Plan v${v.n} came to you before ${by} finished checking it.`;
        return { kind: 'version', text, more: plan.title, plan: { id: plan.id, version: v.n } };
      }
      if (v.decided_line !== m.id) continue;
      // The send-back's message holds the person's words between its first line and its last.
      if (v.decided === 'sent_back')
        return line('Sent back', (m.body as string).split('\n').slice(1, -1).join('\n'));
      // Approved or dropped: the version's chip in its bubble says so (the owner's call of 2026-11-19).
      return null;
    }
  }
  return undefined;
}

/**
 * Who hadn't finished the check when its line was said, for a version that reached the person; null for one that
 * didn't (it has findings) and for a check agreed by a report made by then, as a check that ran late can agree after
 * Gathr sent it on.
 */
function unfinished(v: PlanVersionSummary, line: Message, state: ChatState, crew: Team): string | null {
  if (!v.sent) return null;
  const check = state.tasks.findLast((t) => t.checks === v.id);
  const at = check?.report?.at_us;
  const agreed = check?.state === 'done' && (at == null || at <= (Date.parse(line.time) + 1) * 1000);
  return agreed ? null : (crew.name(check?.owner) ?? 'An agent');
}

/** Who agreed with a version by checking it, as its check line said (0062): their thumbs-up on it (0072). */
export function agreedCheck(v: PlanVersionSummary, state: ChatState, crew: Team): string[] {
  const line = state.messages.find((m) => m.id === v.check_line);
  const by = crew.name(state.tasks.findLast((t) => t.checks === v.id)?.owner);
  return line && by && v.sent && unfinished(v, line, state, crew) === null ? [by] : [];
}
