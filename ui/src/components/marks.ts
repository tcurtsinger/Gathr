/** An agent's state, as the mark on its face (0071), most pressing first. */
export const MARKS = ['needs', 'cant', 'working', 'stopped', 'blocked', 'waiting', 'free'] as const;
export type Mark = (typeof MARKS)[number];

/** Each mark's word: on its card, in Summary and wherever the state is said. */
export const WORD: Record<Mark, string> = {
  needs: 'Needs you',
  cant: 'Can’t run',
  working: 'Working',
  stopped: 'Stopped',
  blocked: 'Blocked',
  waiting: 'Waiting',
  free: 'Free',
};

/** The service's word for a state (0071's answer), as the window names it. */
export const FROM_SERVICE: Record<string, Mark> = {
  needs_you: 'needs',
  cant_run: 'cant',
  working: 'working',
  stopped: 'stopped',
  blocked: 'blocked',
  waiting: 'waiting',
  free: 'free',
};

/** The most pressing of an agent's states: the one its face shows. */
export function pressing(states: (Mark | undefined)[]): Mark | undefined {
  return MARKS.find((m) => states.includes(m));
}

/** What a face's card says: who, and the state with what and since when; with no state, that the agent was removed. */
export interface Card {
  name: string;
  /** "Lead", for the chat's Supervisor. */
  role?: string;
  state?: Mark;
  /** What, after the state's word: "Show cook times · 4 steps". */
  about?: string;
  /** Since when, in ms: the card counts from it when it shows. */
  since?: number;
  /** Waiting: the looks at what it waits on keep failing. */
  failing?: boolean;
}

/** What a row or card says, after what it waits on, when the looks at it keep failing. */
export const CANT_READ = 'Can’t read';

/** How long, as the card says it: "40s", "5m", "1m 12s" while it works, "2h 5m", "3d". */
export function since(ms: number, exact = false): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return exact && s % 60 ? `${m}m ${s % 60}s` : `${m}m`;
  if (m < 1440) return m % 60 ? `${Math.floor(m / 60)}h ${m % 60}m` : `${Math.floor(m / 60)}h`;
  return `${Math.floor(m / 1440)}d`;
}

/** The card's line in its pieces: the state's word, then what, then since when. Free says only its word. */
export function cardParts(
  state: Mark,
  parts: (string | undefined)[],
  began?: number,
  now = Date.now(),
): string[] {
  const when = state !== 'free' && began !== undefined ? since(now - began, state === 'working') : undefined;
  return [WORD[state], ...parts, when].filter((p): p is string => !!p);
}

/** The card's line: "Waiting · PR #18 checks · 5m". */
export const cardLine = (...args: Parameters<typeof cardParts>) => cardParts(...args).join(' · ');
