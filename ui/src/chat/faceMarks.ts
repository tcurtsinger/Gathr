import type { ThreadMessageLike } from '@assistant-ui/react';
import { createContext, useContext, useMemo, useRef } from 'react';
import type { FaceMark } from '../components/Face';
import type { Glance } from './glance';
import type { Custom } from './messages';
import { roleName } from './team';

/**
 * Every face's mark in a chat (0071): one state per agent, the same on each face of it. Its Working ring turns only
 * on the top line and on the agent's latest message. An archived chat has none; an agent no longer in the chat has
 * no mark, and its card says Removed.
 */
export interface ChatMarks {
  /** Each agent of the chat: its mark, still. */
  still: ReadonlyMap<string, FaceMark>;
  /** The same, turning where it works. */
  turning: ReadonlyMap<string, FaceMark>;
  /** Each agent's latest message with its face, by the message's id. */
  latest: ReadonlyMap<string, string>;
  archived: boolean;
}

export const MarksContext = createContext<ChatMarks | null>(null);
export const useMarks = () => useContext(MarksContext);

/** The mark for an agent's face on a message: turning if it is the agent's latest; Removed's card if it left. */
export function useMark(agent: string | undefined, name: string, message?: string): FaceMark | undefined {
  const marks = useMarks();
  return useMemo(() => {
    if (!marks || !agent || marks.archived) return undefined;
    const mark =
      message && marks.latest.get(agent) === message ? marks.turning.get(agent) : marks.still.get(agent);
    return mark ?? { card: { name } };
  }, [marks, agent, name, message]);
}

/** The chat's marks, the same object while no agent's mark, card or latest message changed. */
export function useChatMarks(
  crew: Glance[],
  items: readonly ThreadMessageLike[],
  archived: boolean,
): ChatMarks {
  const latest = latestFaces(items);
  const sig = [
    archived,
    ...crew.map((g) =>
      [g.member.agent, g.member.name, g.member.role, g.standing, g.about, g.since, g.failing].join('|'),
    ),
    ...latest,
  ].join('\n');
  const kept = useRef<{ sig: string; marks: ChatMarks }>(undefined);
  if (kept.current?.sig !== sig) kept.current = { sig, marks: build(crew, latest, archived) };
  return kept.current.marks;
}

function build(crew: Glance[], latest: Map<string, string>, archived: boolean): ChatMarks {
  const still = new Map<string, FaceMark>();
  const turning = new Map<string, FaceMark>();
  // Nobody is anywhere now: an archived chat's faces carry no marks.
  for (const g of archived ? [] : crew) {
    const { agent, name, role } = g.member;
    const card = {
      name,
      role: role === 'supervisor' && crew.length > 1 ? roleName[role] : undefined,
      state: g.standing,
      about: g.about,
      since: g.since,
      failing: g.failing,
    };
    const mark = { state: g.standing, card };
    still.set(agent, mark);
    turning.set(agent, g.standing === 'working' ? { ...mark, spin: true } : mark);
  }
  return { still, turning, latest, archived };
}

/** Each agent's latest message that shows its face, by the message's id. */
function latestFaces(items: readonly ThreadMessageLike[]): Map<string, string> {
  const latest = new Map<string, string>();
  for (const item of items) {
    const custom = (item.metadata?.custom ?? {}) as Custom;
    for (const m of custom.exchange ?? []) if (m.agent) latest.set(m.agent, m.id);
    if (custom.by && custom.head && custom.agent && item.id) latest.set(custom.by, item.id);
  }
  return latest;
}
