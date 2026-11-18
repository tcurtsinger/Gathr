import type { Agent, Member } from '../api';
import type { Person } from '../components/Composer';
import type { FaceMark } from '../components/Face';
import { first, type Picked } from '../components/mention';
import { roleName } from './team';

const MOST = 5;

interface Chat {
  members: Member[];
  agents: Agent[];
  project: boolean;
  /** Not made yet: its first message makes it. */
  fresh: boolean;
  /** The agent of its latest turn: a chat with one agent keeps no list of members (0028). */
  last?: string | null;
  marks?: ReadonlyMap<string, FaceMark>;
}

/**
 * Who a message box's "@" can name (0028, C24): the chat's agents, its Supervisor first; in a chat with no project,
 * then every other agent, which naming brings in, up to five in all. A removed agent is none of them.
 */
export function nameable({ members, agents, project, fresh, last, marks }: Chat): Person[] {
  const known = (id: string) => agents.some((a) => a.id === id);
  const mark = (id: string) => {
    const state = marks?.get(id)?.state;
    return state ? { state } : undefined;
  };
  const sorted = [...members].sort(
    (a, b) => Number(b.role === 'supervisor') - Number(a.role === 'supervisor'),
  );
  const own = agents.find((a) => a.id === last);
  const inChat: Person[] = members.length
    ? sorted
        .filter((m) => known(m.agent))
        .map((m) => ({
          id: m.agent,
          name: m.name,
          meta: members.length > 1 ? roleName[m.role] : undefined,
          mark: mark(m.agent),
        }))
    : !fresh && own
      ? [{ id: own.id, name: own.name, mark: mark(own.id) }]
      : [];
  if (project || inChat.length >= MOST) return inChat;
  const others = agents.filter((a) => !inChat.some((p) => p.id === a.id));
  return [...inChat, ...others.map((a) => ({ id: a.id, name: a.name, out: inChat.length > 0 }))];
}

/** Who gets a message that names nobody: the Supervisor, else the chat's agent so far, else the first agent. */
export function unnamed(people: Person[], members: Member[], last?: string | null): Person | undefined {
  const lead = members.find((m) => m.role === 'supervisor')?.agent;
  return [lead, last].map((id) => people.find((p) => p.id === id)).find(Boolean) ?? people[0];
}

/**
 * Who a message goes to (C24): the first agent picked from the "@" list, else the hint's agent. The window always
 * sends its id, so a typed name is only text; one picked from outside a chat with no project joins it.
 */
export function addressed(
  picks: readonly Picked[],
  people: Person[],
  fallback: Person | undefined,
  fresh: boolean,
) {
  const pick = first(picks.filter((p) => people.some((q) => q.id === p.id)));
  const to = people.find((p) => p.id === pick?.id) ?? fallback;
  return { to, joins: !fresh && !!to?.out };
}
