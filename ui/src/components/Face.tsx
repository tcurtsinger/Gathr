import { createContext, memo, useCallback, useContext, useMemo, useRef } from 'react';
import { drawnOn } from '../faces/backgrounds';
import { Drawing } from '../faces/Drawing';
import { faceOptions, type Gender } from '../faces/options';
import { useCards } from './FaceCard';
import type { Card, Mark } from './marks';
import { StateMark } from './StateMark';
import './face.css';

/** What a face is drawn from: its seed, gender, skin tone and background when known. */
export interface FaceOf {
  id: string;
  name: string;
  /** Its name when it was made, until Shuffle; a face with none is drawn from the name. */
  face_seed?: string;
  gender?: Gender | null;
  skin?: number | null;
  /** #RRGGBB as the person picked it; none is a grey. */
  background?: string | null;
}

/** Every agent the window knows, removed ones too, by id: a place that knows only the id draws the saved face. */
export const KnownFaces = createContext<ReadonlyMap<string, FaceOf>>(new Map());

/** 56 in Settings > Agents' form; the others wherever an agent shows. */
export type FaceSize = 56 | 36 | 28 | 24 | 20 | 16;

/** The agent's state on its face (0071), whether its Working ring turns here, and its card on hover or focus. */
export interface FaceMark {
  state?: Mark;
  spin?: boolean;
  card?: Card;
}

interface Props {
  agent: FaceOf;
  size: FaceSize;
  mark?: FaceMark;
  /** Where no name is shown beside it, the face carries it. */
  label?: string;
}

/**
 * An agent's face (0074): its drawing on the background the person picked, the same everywhere, with its state's
 * mark at 20 and up (0071). Fields given here win (a form's unsaved face); otherwise the face the window knows for
 * the id.
 */
export function Face({ agent, size, mark, label }: Props) {
  const known = useContext(KnownFaces).get(agent.id);
  const shown = agent.face_seed === undefined && known ? known : agent;
  const named = label ? { role: 'img', 'aria-label': label } : { 'aria-hidden': true };
  const ref = useCard(mark?.card);
  return (
    <span
      ref={ref}
      className="face"
      data-agent={agent.id}
      data-size={size}
      data-card={mark?.card ? '' : undefined}
      style={{ background: drawnOn(shown.background) }}
      {...named}
    >
      <Drawn seed={shown.face_seed ?? shown.name} gender={shown.gender ?? null} skin={shown.skin ?? null} />
      {mark?.state && size >= 20 && size <= 36 && (
        <StateMark state={mark.state} size={size} spin={mark.spin} />
      )}
    </span>
  );
}

/**
 * The card shows while the face is pointed at or has keyboard focus: on its own button, when that holds this face
 * alone (its hover tint lies over the face).
 */
function useCard(card?: Card) {
  const cards = useCards();
  const latest = useRef(card);
  latest.current = card;
  const has = !!card && !!cards;
  // On the element itself: a face moved into its button, once the chat's team is known, is a new element.
  return useCallback(
    (face: HTMLSpanElement | null) => {
      if (!has || !face || !cards) return;
      const show = () => latest.current && cards.show(face, latest.current);
      const hide = () => cards.hide(face);
      const host = face.parentElement?.closest<HTMLElement>('button, a[href], [tabindex]');
      const alone = host && host.querySelectorAll('.face[data-card]').length === 1 ? host : null;
      (alone ?? face).addEventListener('pointerenter', show);
      (alone ?? face).addEventListener('pointerleave', hide);
      alone?.addEventListener('focusin', show);
      alone?.addEventListener('focusout', hide);
      return () => {
        hide();
        (alone ?? face).removeEventListener('pointerenter', show);
        (alone ?? face).removeEventListener('pointerleave', hide);
        alone?.removeEventListener('focusin', show);
        alone?.removeEventListener('focusout', hide);
      };
    },
    [has, cards],
  );
}

/**
 * Drawn again only when what it is drawn from changes: a streamed token, a new message or a background dragged in
 * the picker never redraws one.
 */
const Drawn = memo(function Drawn({
  seed,
  gender,
  skin,
}: {
  seed: string;
  gender: Gender | null;
  skin: number | null;
}) {
  const options = useMemo(() => faceOptions(seed, { gender, skin }), [seed, gender, skin]);
  return <Drawing options={options} />;
});

/** Faces overlapped into one compact element, in the order given, each cut out of the surface behind it. */
export function FaceStack({ agents }: { agents: { agent: string; name: string }[] }) {
  return (
    <span className="face-stack" aria-hidden="true">
      {agents.map((a) => (
        <Face key={a.agent} agent={{ id: a.agent, name: a.name }} size={20} />
      ))}
    </span>
  );
}
