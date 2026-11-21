import { type CSSProperties, createContext, useContext } from 'react';
import { type Chosen, lineAt, lookOf } from './pictures';
import './face.css';

/** What a picture is drawn from: the agent's id and its choices, when known. */
export interface FaceOf extends Chosen {
  id: string;
  name: string;
}

/** Every agent the window knows, removed ones too, by id: a place that knows only the id draws the saved picture. */
export const KnownFaces = createContext<ReadonlyMap<string, FaceOf>>(new Map());

/** 56 in Settings > Agents' form and the new chat; the others wherever an agent shows. */
export type FaceSize = 56 | 36 | 28 | 24 | 20 | 16;

interface Props {
  agent: FaceOf;
  size: FaceSize;
  /** Where no name is shown beside it, the picture carries it. */
  label?: string;
}

/**
 * An agent's picture: its shape and eyes in lines of its colour, the same everywhere. At 20 and under a shape with a
 * simpler drawing uses it. Choices given here win (a form's unsaved pick); otherwise the ones the window knows.
 */
export function Face({ agent, size, label }: Props) {
  const known = useContext(KnownFaces).get(agent.id);
  const shown = agent.shape === undefined && known ? known : agent;
  const { shape, eyes, colour } = lookOf(agent.id, shown);
  const named = label ? { role: 'img', 'aria-label': label } : { 'aria-hidden': true };
  const line = lineAt(size);
  const ink = { '--ink': colour.dark, '--ink-l': colour.light } as CSSProperties;
  return (
    <span className="face" data-agent={agent.id} data-size={size} style={ink} {...named}>
      <svg viewBox="0 0 48 48" aria-hidden="true">
        <g fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round">
          <path d={size <= 20 && 'small' in shape ? shape.small : shape.d} strokeWidth={line} />
          <g transform={`translate(24 ${shape.y}) scale(${shape.s})`} strokeWidth={line / shape.s}>
            {eyes.d.map((d) => (
              <path key={d} d={d} />
            ))}
          </g>
        </g>
      </svg>
    </span>
  );
}
