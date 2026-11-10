import type { Model } from '../api';

/** A model the editor offers: null is the engine's own default. */
export interface Choice {
  id: string | null;
  label: string;
}

const DEFAULT = 'default';

/** Latest first, then Engine default, then numbered models, each in the engine's order. */
export function choices(models: Model[]): Choice[] {
  // The family from the alias: signed in, the engine names it by today's version ("Opus 5.5").
  const family = (id: string) => {
    const name = id.startsWith('latest:') ? id.slice('latest:'.length) : id;
    return name.charAt(0).toUpperCase() + name.slice(1);
  };
  const aliases = models.filter((m) => m.latest).map((m) => ({ id: m.id, label: `${family(m.id)} Latest` }));
  const pinned = models
    .filter((m) => m.id !== DEFAULT && !m.latest)
    .map((m) => ({ id: m.id, label: m.name }));
  return [...aliases, { id: null, label: 'Engine default' }, ...pinned];
}

/** What a new agent gets: its default model's Latest family, else the first Latest or Engine default. */
export const firstLatest = (models: Model[]) => {
  const resolved = models.find((m) => m.id === DEFAULT)?.resolved;
  return (
    (resolved ? models.find((m) => m.latest && m.resolved === resolved)?.id : undefined) ??
    models.find((m) => m.latest)?.id ??
    null
  );
};

/** The effort levels the chosen model supports; for Engine default, the engine's default model's. */
export const efforts = (models: Model[], model: string | null) =>
  models.find((m) => m.id === (model ?? DEFAULT))?.efforts ?? [];

/** How the chosen model reads above the list; a saved model the engine no longer lists says so. */
export function chosen(models: Model[], model: string | null) {
  const choice = choices(models).find((c) => c.id === model);
  return choice?.label ?? `${model} / Unavailable`;
}
