import { useRef } from 'react';
import { Button } from '@/components/ui/button';
import { drawFace, Face, SKIN } from '../components/Face';
import { BackgroundField } from './BackgroundField';

export interface FaceChange {
  face_seed?: string;
  skin?: number;
  background?: string;
  picture?: string | null;
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
  picture: string | null;
  remembered: string | null;
  onChange: (change: FaceChange) => void;
}

/**
 * Settings > Agents' Face, Skin tone and Background (0074), saved with the form. A tone never set shows what the
 * seed picked, and choosing one keeps it from then on. Shuffle picks a new seed and keeps the tone shown, which sets
 * it, so a face never changes tone that way. Upload puts a picture from the computer in place of the drawn face, and
 * Remove brings the drawn one back.
 */
export function FaceFields({ id, name, seed, skin, background, picture, remembered, onChange }: Props) {
  const picked = drawFace(seed, skin).skin;
  const file = useRef<HTMLInputElement>(null);
  return (
    <>
      <fieldset className="agent-field">
        <legend>Face</legend>
        <span className="face-pick">
          <Face
            agent={{ id, name, face_seed: seed, skin, background, picture }}
            size={56}
            label={name || undefined}
          />
          <Button
            type="button"
            variant="outline"
            onClick={() => onChange({ face_seed: Math.random().toString(36).slice(2, 10), skin: picked })}
          >
            Shuffle
          </Button>
          <Button type="button" variant="outline" onClick={() => file.current?.click()}>
            Upload
          </Button>
          {picture && (
            <Button type="button" variant="outline" onClick={() => onChange({ picture: null })}>
              Remove
            </Button>
          )}
          <input
            ref={file}
            type="file"
            accept="image/*"
            hidden
            onChange={(e) => {
              const chosen = e.target.files?.[0];
              e.target.value = '';
              if (chosen) shrink(chosen).then((p) => onChange({ picture: p }));
            }}
          />
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

/** The picture's middle square at 256 px, so what the service keeps stays small. */
async function shrink(file: File) {
  const image = await createImageBitmap(file);
  const side = Math.min(image.width, image.height);
  const canvas = document.createElement('canvas');
  canvas.width = 256;
  canvas.height = 256;
  const x = (image.width - side) / 2;
  const y = (image.height - side) / 2;
  canvas.getContext('2d')?.drawImage(image, x, y, side, side, 0, 0, 256, 256);
  return canvas.toDataURL('image/webp', 0.9);
}
