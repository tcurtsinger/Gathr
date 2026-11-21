/** An agent's state, as its mark (0071), most pressing first. */
const MARKS = ['needs', 'cant', 'working', 'stopped', 'blocked', 'waiting', 'free'] as const;
export type Mark = (typeof MARKS)[number];

/** Each mark's word, wherever the state is said. */
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

/** How long: "40s", "5m", "1m 12s" while it works, "2h 5m", "3d". */
export function since(ms: number, exact = false): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return exact && s % 60 ? `${m}m ${s % 60}s` : `${m}m`;
  if (m < 1440) return m % 60 ? `${Math.floor(m / 60)}h ${m % 60}m` : `${Math.floor(m / 60)}h`;
  return `${Math.floor(m / 1440)}d`;
}
