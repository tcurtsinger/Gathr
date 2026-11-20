import { createAvatar } from '@dicebear/core';
import * as dylan from '@dicebear/dylan';
import { createContext, memo, useContext, useMemo } from 'react';
import { drawnOn } from './backgrounds';
import type { Mark } from './marks';
import { StateMark } from './StateMark';
import './face.css';

/** Six skin tones, light to deep (skin tone 1 to 6). */
export const SKIN = ['FFE1CC', 'EDC1A7', 'D8A184', 'C28266', 'AA654B', '904A34'];
/** Dylan's moods but angry, sad and confused: no agent looks upset for good. */
const MOODS: dylan.Options['mood'] = ['happy', 'neutral', 'superHappy', 'hopeful'];

/**
 * DiceBear's Dylan for a seed, with no background, and its skin tone: the person's, or the seed's pick. The seed
 * picks everything else.
 */
export function drawFace(seed: string, skin: number | null) {
  const made = createAvatar(dylan, {
    seed,
    mood: MOODS,
    skinColor: skin ? [SKIN[skin - 1]] : SKIN,
    backgroundColor: [],
    clip: false,
  });
  const picked = String(made.toJson().extra.skinColor).slice(1).toUpperCase();
  return { src: made.toDataUri(), skin: SKIN.indexOf(picked) + 1 };
}

/** What a face is drawn from: its seed, skin tone and background when known. */
export interface FaceOf {
  id: string;
  name: string;
  /** Its name when it was made, until Shuffle; a face with none is drawn from the name. */
  face_seed?: string;
  skin?: number | null;
  /** #RRGGBB as the person picked it; none is a grey. */
  background?: string | null;
  /** A picture the person uploaded, a data URL: shown in place of the drawing. */
  picture?: string | null;
}

/** Every agent the window knows, removed ones too, by id: a place that knows only the id draws the saved face. */
export const KnownFaces = createContext<ReadonlyMap<string, FaceOf>>(new Map());

/** 56 in Settings > Agents' form; the others wherever an agent shows. */
export type FaceSize = 56 | 36 | 28 | 24 | 20 | 16;

/** The agent's state on its face (0071), and whether its Working ring turns here. */
export interface FaceMark {
  state?: Mark;
  spin?: boolean;
}

interface Props {
  agent: FaceOf;
  size: FaceSize;
  mark?: FaceMark;
  /** Where no name is shown beside it, the face carries it. */
  label?: string;
}

/**
 * An agent's face (0074): its picture, or its drawing on the background the person picked, the same everywhere,
 * with its state's mark at 20 and up (0071). Fields given here win (a form's unsaved face); otherwise the face the
 * window knows for the id.
 */
export function Face({ agent, size, mark, label }: Props) {
  const known = useContext(KnownFaces).get(agent.id);
  const shown = agent.face_seed === undefined && known ? known : agent;
  const named = label ? { role: 'img', 'aria-label': label } : { 'aria-hidden': true };
  return (
    <span
      className="face"
      data-agent={agent.id}
      data-size={size}
      style={{ background: drawnOn(shown.background) }}
      {...named}
    >
      {shown.picture ? (
        <img src={shown.picture} alt="" draggable={false} />
      ) : (
        <Drawn seed={shown.face_seed ?? shown.name} skin={shown.skin ?? null} />
      )}
      {mark?.state && size >= 20 && size <= 36 && (
        <StateMark state={mark.state} size={size} spin={mark.spin} />
      )}
    </span>
  );
}

/**
 * Drawn again only when what it is drawn from changes: a streamed token, a new message or a background dragged in
 * the picker never redraws one.
 */
const Drawn = memo(function Drawn({ seed, skin }: { seed: string; skin: number | null }) {
  const src = useMemo(() => drawFace(seed, skin).src, [seed, skin]);
  return <img src={src} alt="" draggable={false} />;
});
