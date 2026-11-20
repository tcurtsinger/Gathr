import { createContext, useContext, useMemo, useRef } from 'react';
import type { FaceMark } from '../components/Face';
import type { Glance } from './glance';
import { roleName } from './team';

/**
 * Every face's mark in a chat (0071): one state per agent, the same on each face of it, and its Working ring turns
 * on every one (the owner's call of 2026-11-19: a ring standing still on an older message read as stuck). An archived
 * chat has none; an agent no longer in the chat has no mark, and its card says Removed.
 */
export interface ChatMarks {
  /** Each agent of the chat: its mark. */
  faces: ReadonlyMap<string, FaceMark>;
  archived: boolean;
}

export const MarksContext = createContext<ChatMarks | null>(null);
export const useMarks = () => useContext(MarksContext);

/** The mark for an agent's face on a message; Removed's card if it left. */
export function useMark(agent: string | undefined, name: string): FaceMark | undefined {
  const marks = useMarks();
  return useMemo(() => {
    if (!marks || !agent || marks.archived) return undefined;
    return marks.faces.get(agent) ?? { card: { name } };
  }, [marks, agent, name]);
}

/** The chat's marks, the same object while no agent's mark or card changed. */
export function useChatMarks(crew: Glance[], archived: boolean): ChatMarks {
  const sig = [
    archived,
    ...crew.map((g) =>
      [g.member.agent, g.member.name, g.member.role, g.standing, g.about, g.since, g.failing].join('|'),
    ),
  ].join('\n');
  const kept = useRef<{ sig: string; marks: ChatMarks }>(undefined);
  if (kept.current?.sig !== sig) kept.current = { sig, marks: build(crew, archived) };
  return kept.current.marks;
}

function build(crew: Glance[], archived: boolean): ChatMarks {
  const faces = new Map<string, FaceMark>();
  // Nobody is anywhere now: an archived chat's faces carry no marks.
  for (const g of archived ? [] : crew) {
    const { agent, name, role } = g.member;
    const card = {
      name,
      role: role === 'supervisor' ? roleName[role] : undefined,
      state: g.standing,
      about: g.about,
      since: g.since,
      failing: g.failing,
    };
    faces.set(agent, { state: g.standing, card, spin: g.standing === 'working' });
  }
  return { faces, archived };
}
