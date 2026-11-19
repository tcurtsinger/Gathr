import type { Agent, Member } from '../api';
import type { Person } from '../components/Composer';
import type { FaceMark } from '../components/Face';
import type { Picked } from '../components/mention';
import { roleName } from './team';

const MOST = 5;

interface Chat {
  members: Member[];
  agents: Agent[];
  project: boolean;
  /** Not made yet: its first message makes it. */
  fresh: boolean;
  /** The agent of its latest turn, for a chat with no list of members yet. */
  last?: string | null;
  marks?: ReadonlyMap<string, FaceMark>;
}

/**
 * Who a message box's "@" can name (0028): the chat's agents, its Supervisor first; in a chat with no project,
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
          meta: roleName[m.role],
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

/** Who gets a message, as the sign by Send names them: those not in the chat yet join it. */
export interface Recipient {
  id: string;
  name: string;
  joins?: boolean;
}

/**
 * Who a message goes to (0028): every agent picked from the "@" list, in the order the names stand, each once; with
 * none picked, the hint's agent. A reply (0076) goes to the agent it answers first, then each one picked. The window
 * sends the first one's id and the picks, so a typed name is only text; one picked from outside a chat with no project
 * joins it.
 */
export function addressed(
  picks: readonly Picked[],
  people: Person[],
  fallback: Person | undefined,
  { fresh, reply = false }: { fresh: boolean; reply?: boolean },
): Recipient[] {
  const picked = [...picks].sort((a, b) => a.at - b.at).flatMap((p) => people.filter((q) => q.id === p.id));
  const all = reply || !picked.length ? [fallback, ...picked] : picked;
  const once = all.filter((p, i): p is Person => !!p && all.findIndex((q) => q?.id === p.id) === i);
  return once.map((p) => ({ id: p.id, name: p.name, joins: !fresh && !!p.out }));
}
