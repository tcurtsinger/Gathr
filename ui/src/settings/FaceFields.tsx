import { Button } from '@/components/ui/button';
import { drawFace, Face, SKIN } from '../components/Face';
import { BackgroundField } from './BackgroundField';

export interface FaceChange {
  face_seed?: string;
  skin?: number;
  background?: string;
  /** Background's last colour not among the twelve, kept with the form's edits only. */
  custom?: string;
}

interface Props {
  /** The agent's id; empty for a new one. */
  id: string;
  name: string;
  seed: string;
  skin: number | null;
  background: string | null;
  remembered: string | null;
  onChange: (change: FaceChange) => void;
}

/**
 * Settings > Agents' Face, Skin tone and Background (0074), saved with the form. A tone never set shows what the
 * seed picked, and choosing one keeps it from then on. Shuffle picks a new seed and keeps the tone shown, which sets
 * it, so a face never changes tone that way.
 */
export function FaceFields({ id, name, seed, skin, background, remembered, onChange }: Props) {
  const picked = drawFace(seed, skin).skin;
  return (
    <>
      <fieldset className="agent-field">
        <legend>Face</legend>
        <span className="face-pick">
          <Face agent={{ id, name, face_seed: seed, skin, background }} size={56} label={name || undefined} />
          <Button
            type="button"
            variant="outline"
            onClick={() => onChange({ face_seed: Math.random().toString(36).slice(2, 10), skin: picked })}
          >
            Shuffle
          </Button>
        </span>
      </fieldset>
      <fieldset className="agent-field">
        <legend>Skin tone</legend>
        <span className="swatches">
          {SKIN.map((colour, i) => (
            <button
              type="button"
              key={colour}
              className="swatch"
              style={{ background: `#${colour}` }}
              aria-label={`Skin tone ${i + 1}`}
              aria-pressed={picked === i + 1}
              onClick={() => onChange({ skin: i + 1 })}
            />
          ))}
        </span>
      </fieldset>
      <BackgroundField background={background} remembered={remembered} onChange={onChange} />
    </>
  );
}
