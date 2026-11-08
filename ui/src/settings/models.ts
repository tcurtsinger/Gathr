import type { Model } from '../api';

/** A model the editor offers: null is the engine's own default. */
export interface Choice {
  id: string | null;
  label: string;
}

const DEFAULT = 'default';

/** A family alias (opus, sonnet) always means that family's newest model (0025). */
const latest = (id: string) => id !== DEFAULT && !/\d/.test(id);

/** Latest first, then Engine default, then numbered models, each in the engine's order. */
export function choices(models: Model[]): Choice[] {
  // The family from the alias: signed in, the engine names it by today's version ("Opus 5.5").
  const family = (id: string) => id.charAt(0).toUpperCase() + id.slice(1);
  const aliases = models
    .filter((m) => latest(m.id))
    .map((m) => ({ id: m.id, label: `${family(m.id)} Latest` }));
  const pinned = models
    .filter((m) => m.id !== DEFAULT && !latest(m.id))
    .map((m) => ({ id: m.id, label: m.name }));
  return [...aliases, { id: null, label: 'Engine default' }, ...pinned];
}

/** What a new agent gets: its engine's first Latest, else the engine's default. */
export const firstLatest = (models: Model[]) => models.find((m) => latest(m.id))?.id ?? null;

/** The effort levels the chosen model supports; for Engine default, the engine's default model's. */
export const efforts = (models: Model[], model: string | null) =>
  models.find((m) => m.id === (model ?? DEFAULT))?.efforts ?? [];

/** How the chosen model reads above the list; a saved model the engine no longer lists says so. */
export function chosen(models: Model[], model: string | null) {
  const choice = choices(models).find((c) => c.id === model);
  return choice?.label ?? `${model} / Unavailable`;
}
