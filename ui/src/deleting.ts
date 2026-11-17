import { Failure } from './api';
import { list } from './chat/team';

/** What deleting a project or an agent does (0069), as the service counts it before and answers after. */
export interface DeleteCounts {
  /** Chats and projects deleted; a project's own delete counts the project itself. */
  chats: number;
  projects: number;
  /** The agent's open tasks cancelled in chats that stay. */
  tasks: number;
  /** Who becomes lead where the agent led, by name. */
  leads: string[];
}

export type Deletable = 'project' | 'agent';

const count = (n: number, noun: string) => `${n} ${noun}${n === 1 ? '' : 's'}`;

/**
 * The confirm's words (the owner's, 0069): what is deleted, then the tasks cancelled, then who becomes
 * lead, then "Files are kept." when anything with files is deleted. The project being deleted is named
 * in the question, so its body counts only its chats.
 */
export function deleteBody(kind: Deletable, c: DeleteCounts): string {
  const projects = kind === 'project' ? 0 : c.projects;
  const deleted = [projects && count(projects, 'project'), c.chats && count(c.chats, 'chat')].filter(Boolean);
  const files = kind === 'project' || deleted.length > 0;
  return [
    deleted.length > 0 && `${deleted.join(' and ')} will be deleted.`,
    c.tasks > 0 && `${count(c.tasks, 'task')} will be cancelled.`,
    c.leads.length === 1 && `${c.leads[0]} becomes lead.`,
    c.leads.length > 1 && `${list(c.leads)} become leads.`,
    files && 'Files are kept.',
  ]
    .filter(Boolean)
    .join(' ');
}

/**
 * What a delete couldn't remove (`left`, each "<why> (ref <reference>)"), as one failure: the first
 * reference opens, and a line with another reference keeps it in its words.
 */
export function leftFailure(left: string[]): Failure | null {
  if (left.length === 0) return null;
  const refs = left.map((l) => l.match(/ \(ref ([^)]+)\)$/)?.[1] ?? null);
  const ref = refs.find((r) => r !== null) ?? null;
  const why = left.map((l, i) =>
    refs[i] !== null && refs[i] === ref ? l.slice(0, l.lastIndexOf(' (ref ')) : l,
  );
  return new Failure('Some files weren’t deleted', why.join('; '), ref);
}
