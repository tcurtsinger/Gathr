import { Button } from '@/components/ui/button';
import { Face } from '../components/Face';
import { faceOptions, type Gender, SKIN } from '../faces/options';
import { BackgroundField } from './BackgroundField';

export interface FaceChange {
  face_seed?: string;
  gender?: Gender;
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
  gender: Gender | null;
  skin: number | null;
  background: string | null;
  remembered: string | null;
  onChange: (change: FaceChange) => void;
}

/**
 * Settings > Agents' Face, Gender, Skin tone and Background (0074), saved with the form. A gender or tone never set shows what
 * the seed picked, and choosing one keeps it from then on. Shuffle picks a new seed and keeps the gender and tone
 * shown, which sets them, so a face never changes gender or tone that way.
 */
export function FaceFields({ id, name, seed, gender, skin, background, remembered, onChange }: Props) {
  const picked = faceOptions(seed, { gender, skin });
  return (
    <>
      <fieldset className="agent-field">
        <legend>Face</legend>
        <span className="face-pick">
          <Face
            agent={{ id, name, face_seed: seed, gender, skin, background }}
            size={56}
            label={name || undefined}
          />
          <Button
            type="button"
            variant="outline"
            onClick={() =>
              onChange({
                face_seed: Math.random().toString(36).slice(2, 10),
                gender: picked.gender,
                skin: picked.skin,
              })
            }
          >
            Shuffle
          </Button>
        </span>
      </fieldset>
      <label className="agent-field">
        Gender
        <select value={picked.gender} onChange={(e) => onChange({ gender: e.target.value as Gender })}>
          <option value="man">Man</option>
          <option value="woman">Woman</option>
        </select>
      </label>
      <fieldset className="agent-field">
        <legend>Skin tone</legend>
        <span className="swatches">
          {SKIN.map((colour, i) => (
            <button
              type="button"
              key={colour}
              className="swatch"
              style={{ background: colour }}
              aria-label={`Skin tone ${i + 1}`}
              aria-pressed={picked.skin === i + 1}
              onClick={() => onChange({ skin: i + 1 })}
            />
          ))}
        </span>
      </fieldset>
      <BackgroundField background={background} remembered={remembered} onChange={onChange} />
    </>
  );
}
