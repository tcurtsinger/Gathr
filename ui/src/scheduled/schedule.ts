/** Words an agent scheduled in a chat, as `GET /api/schedules` lists them: `when` is the service's own words for it. */
export interface Schedule {
  id: string;
  chat: string;
  chat_title: string;
  /** Its chat's project's name, when it has one. */
  project: string | null;
  agent: string;
  words: string;
  when: string;
  enabled: boolean;
  /** When it next runs; none while disabled. */
  next: string | null;
}

/** One time it came due: when it ran (a skipped one, when it was due), its state and one line about it. */
export interface ScheduleRun {
  id: string;
  at: string;
  state: 'running' | 'done' | 'late' | 'skipped' | 'failed' | 'stopped';
  line: string | null;
  /** The words' message in its chat; none when it was skipped. */
  message: string | null;
}

export interface ScheduleShown extends Schedule {
  runs: ScheduleRun[];
}
