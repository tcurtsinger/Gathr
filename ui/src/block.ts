import type { ProposedAgent, ProposedProject } from './room/Proposal';

/** A cell of a shown table: what the agent gave, as its column's `as` wants. */
export type Cell = string | number | boolean | null;

/** A table, chart, flow, math, diagram or page an agent showed with `show`, as the service keeps it. A later kind adds one more field. */
export interface Block {
  kind: string;
  title?: string;
  table?: {
    columns: {
      key: string;
      label: string;
      as?: 'text' | 'number' | 'money' | 'percent' | 'change' | 'date' | 'link' | 'yes_no';
      unit?: string;
      currency?: string;
      decimals?: number;
      up_is_good?: boolean;
    }[];
    rows: Record<string, Cell>[];
  };
  chart?: {
    label: string;
    value: string;
    points: number[];
    change?: string;
    /** What `change` is measured against: "Oct", "last year". */
    vs?: string;
    /** A name for each point, as many as the points: "Jan", "Feb"… */
    labels?: string[];
    style?: 'line' | 'area' | 'bars';
    up_is_good?: boolean;
  };
  flow?: {
    nodes: { id: string; label: string; after?: string[]; state?: 'done' | 'active' | 'pending' }[];
  };
  math?: {
    label?: string;
    steps: { expression: string; note?: string }[];
  };
  /** Mermaid source or a picture's full path: one of the two. */
  diagram?: { mermaid?: string; file?: string };
  /** An HTML page's full path, shown sealed at `height` (360 unless set). */
  page?: { file: string; height?: number };
  /** A project the assistant proposed with `propose_project`; once made, what was made. */
  project?: ProposedProject;
  /** An agent the assistant proposed with `propose_agent`; once added, the agent as it was added. */
  agent?: ProposedAgent;
  /** The message that holds it: not kept in the body, set as the chat reads. */
  message?: string;
}
