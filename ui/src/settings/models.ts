import type { Model } from '../api';

/** A model the editor offers, by its exact id. */
export interface Choice {
  id: string;
  label: string;
}

const DEFAULT = 'default';

/**
 * The exact models, in the engine's order, by the engine's own names ("Opus 5.5"). Never the engine's
 * default or a family's Latest, which become another model when one comes out (the owner, 2026-11-12:
 * a model is chosen, and changed by hand).
 */
export const choices = (models: Model[]): Choice[] =>
  models.filter((m) => m.id !== DEFAULT && !m.latest).map((m) => ({ id: m.id, label: m.name }));

/** The row an agent's model is saved as: null is the engine's default. */
const saved = (models: Model[], model: string | null) => models.find((m) => m.id === (model ?? DEFAULT));

/** The exact model an agent runs today: its own, or the one its engine default or Latest means now. */
export function runs(models: Model[], model: string | null): string | null {
  const exact = saved(models, model)?.resolved ?? model;
  return choices(models).some((c) => c.id === exact) ? (exact ?? null) : null;
}

/** What a new agent gets: the exact model its engine's default means today, else the first listed. */
export const firstChoice = (models: Model[]) => runs(models, null) ?? choices(models)[0]?.id ?? null;

/** The effort levels the chosen model supports; for an engine default, the engine's default model's. */
export const efforts = (models: Model[], model: string | null) => saved(models, model)?.efforts ?? [];

/**
 * How the chosen model reads above the list. An agent saved before on its engine's default or a Latest
 * says which model that is today; a saved model the engine no longer lists says so.
 */
export function chosen(models: Model[], model: string | null) {
  const exact = choices(models).find((c) => c.id === runs(models, model));
  if (exact && exact.id === model) return exact.label;
  if (exact) return `${exact.label} / ${model === null ? 'Engine default' : 'Latest'}`;
  return model === null ? 'Engine default' : `${model} / Unavailable`;
}
