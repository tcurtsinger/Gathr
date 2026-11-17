import { useId } from 'react';
import { Brows, Eyes, Glasses } from './eyes';
import { Hair } from './hair';
import { Hat } from './hat';
import { Ear, Head } from './head';
import { Mouth, Nose } from './mouth';
import type { FaceOptions } from './options';
import { Shirt } from './shirt';

/**
 * A face on a 100 by 100 square, with no background. react-nice-avatar lays its parts out as HTML boxes; here
 * each part is a nested <svg> at the box the library's CSS gives it, so the shapes and their places are its own:
 * the face's box is the lower 90 of the square, and the eyes, ears, nose and mouth sit in a box of the same size
 * 3 to the right and 27 lower, where a part with no left of its own is centred, as in the library's flexbox.
 * In the library's order: face, hat or hair, brows, eyes, glasses, ear, nose, mouth, then the shirt over all.
 */
export function Drawing({ options: o }: { options: FaceOptions }) {
  // Masks need ids unique on the page: a face drawn twice must never use another's.
  const id = `face${useId().replace(/[^\w-]/g, '')}`;
  return (
    <svg viewBox="0 0 100 100" aria-hidden="true" focusable="false">
      <Head color={o.faceColor} id={id} />
      {o.hatStyle === 'none' ? (
        <Hair style={o.hairStyle} color={o.hairColor} />
      ) : (
        <Hat style={o.hatStyle} color={o.hatColor} />
      )}
      <Brows style={o.eyeBrowStyle} />
      <Eyes style={o.eyeStyle} />
      <Glasses style={o.glassesStyle} />
      <Ear size={o.earSize} color={o.faceColor} />
      <Nose style={o.noseStyle} />
      <Mouth style={o.mouthStyle} id={id} />
      <Shirt style={o.shirtStyle} color={o.shirtColor} />
    </svg>
  );
}
