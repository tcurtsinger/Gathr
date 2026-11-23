/** The AI app outside Gathr that added a skill, a tool, an agent or a project, if one did (`gathr connect`). */
export interface Added {
  added_by?: string | null;
}

/** "Added by <app>" in the line under a title, for a skill, tool, agent or project an AI app outside Gathr added. */
export function AddedBy({ app }: { app?: string | null }) {
  return app ? <span className="added-by">Added by {app}</span> : null;
}
