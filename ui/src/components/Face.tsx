import { createContext, memo, useContext, useMemo } from 'react';
import { Drawing } from '../faces/Drawing';
import { faceOptions, type Gender, hash } from '../faces/options';
import './face.css';

/** What a face is drawn from: the agent's id gives its colour; its seed, gender and skin tone when known. */
export interface FaceOf {
  id: string;
  name: string;
  /** Its name when it was made, until Shuffle; a face with none is drawn from the name. */
  face_seed?: string;
  gender?: Gender | null;
  skin?: number | null;
}

/** Every agent the window knows, removed ones too, by id: a place that knows only the id draws the saved face. */
export const KnownFaces = createContext<ReadonlyMap<string, FaceOf>>(new Map());

/** 56 in Settings > Agents' form; the others wherever an agent shows. */
export type FaceSize = 56 | 36 | 28 | 24 | 20 | 16;

/** The agent's colour, one of five, from its id: there is no setting for it (0074). */
export const hueOf = (id: string) => (hash(id, 'hue') % 5) + 1;

interface Props {
  agent: FaceOf;
  size: FaceSize;
  /** A turn of its is running: an attention ring turns around it. */
  busy?: boolean;
  /** Where no name is shown beside it, the face carries it. */
  label?: string;
}

/**
 * An agent's face (0074): its drawing on a pale tint of its colour, the same everywhere. Fields given here win
 * (a form's unsaved face); otherwise the face the window knows for the id.
 */
export function Face({ agent, size, busy, label }: Props) {
  const known = useContext(KnownFaces).get(agent.id);
  const shown = agent.face_seed === undefined && known ? known : agent;
  return (
    <Drawn
      seed={shown.face_seed ?? shown.name}
      gender={shown.gender ?? null}
      skin={shown.skin ?? null}
      id={agent.id}
      size={size}
      busy={busy}
      label={label}
    />
  );
}

/** Drawn again only when what it is drawn from changes: a streamed token or a new message never redraws one. */
const Drawn = memo(function Drawn(props: {
  id: string;
  seed: string;
  gender: Gender | null;
  skin: number | null;
  size: FaceSize;
  busy?: boolean;
  label?: string;
}) {
  const { id, seed, gender, skin, size, busy, label } = props;
  const options = useMemo(() => faceOptions(seed, { gender, skin }), [seed, gender, skin]);
  const named = label ? { role: 'img', 'aria-label': label } : { 'aria-hidden': true };
  return (
    <span
      className="face"
      data-agent={id}
      data-size={size}
      data-hue={hueOf(id)}
      data-busy={busy || undefined}
      {...named}
    >
      <Drawing options={options} />
    </span>
  );
});

/** Faces overlapped into one compact element, in the order given, each cut out of the surface behind it. */
export function FaceStack({
  agents,
  needs,
}: {
  agents: { agent: string; name: string; busy?: boolean }[];
  /** Someone needs the person: one attention dot on the stack's corner. */
  needs?: boolean;
}) {
  return (
    <span className="face-stack" data-needs={needs || undefined} aria-hidden="true">
      {agents.map((a) => (
        <Face key={a.agent} agent={{ id: a.agent, name: a.name }} size={20} busy={a.busy} />
      ))}
      {needs && <i className="attention" />}
    </span>
  );
}
