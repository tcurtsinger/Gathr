/** A place in Memory: `you`, `project:<id>`, `agent:<id>` or `tool:<id>`, with how many notes it holds. */
export interface MemoryPlace {
  id: string;
  kind: 'you' | 'project' | 'agent' | 'tool';
  name: string;
  count: number;
}

/** A note: who added it (an agent's name, `You`, or nobody for a line written in the file) and when. */
export interface MemoryNote {
  text: string;
  by: string | null;
  when: string | null;
}

export interface MemoryShown {
  id: string;
  name: string;
  path: string;
  notes: MemoryNote[];
}
