import type { CSSProperties, ReactNode } from 'react';
import { Button } from '@/components/ui/button';
import { Face } from '../components/Face';
import { type Chosen, COLOURS, EYES, lookOf, SHAPES, shuffled } from '../components/pictures';

interface Props {
  /** The agent's id; for a new one, the id its picture is worked out from until it is saved. */
  id: string;
  name: string;
  picked: Chosen;
  onChange: (change: Chosen) => void;
}

/**
 * Settings > Agents' Picture, Shape, Eyes and Colour, saved with the form. Each row shows the twelve choices, only
 * what it picks, in the agent's colour, the one shown ringed. Shuffle picks all three anew.
 */
export function FaceFields({ id, name, picked, onChange }: Props) {
  const look = lookOf(id, picked);
  const ink = { '--ink': look.colour.dark, '--ink-l': look.colour.light } as CSSProperties;
  return (
    <>
      <fieldset className="agent-field">
        <legend>Picture</legend>
        <span className="face-pick">
          <Face agent={{ id, name, ...picked }} size={56} label={name || undefined} />
          <Button type="button" variant="outline" onClick={() => onChange(shuffled())}>
            Shuffle
          </Button>
        </span>
      </fieldset>
      <fieldset className="agent-field">
        <legend>Shape</legend>
        <span className="picks" style={ink}>
          {SHAPES.map((s) => (
            <Pick
              key={s.key}
              part="shape"
              name={s.name}
              on={s === look.shape}
              onClick={() => onChange({ shape: s.key })}
            >
              <path d={s.d} strokeWidth={3} />
            </Pick>
          ))}
        </span>
      </fieldset>
      <fieldset className="agent-field">
        <legend>Eyes</legend>
        <span className="picks" style={ink}>
          {EYES.map((e) => (
            <Pick
              key={e.key}
              part="eyes"
              name={e.name}
              on={e === look.eyes}
              onClick={() => onChange({ eyes: e.key })}
            >
              <g transform="translate(24 24) scale(1.45)" strokeWidth={3 / 1.45}>
                {e.d.map((d) => (
                  <path key={d} d={d} />
                ))}
              </g>
            </Pick>
          ))}
        </span>
      </fieldset>
      <fieldset className="agent-field">
        <legend>Colour</legend>
        <span className="picks colours">
          {COLOURS.map((c) => (
            <button
              type="button"
              key={c.key}
              className="swatch"
              style={{ '--ink': c.dark, '--ink-l': c.light } as CSSProperties}
              aria-label={c.name}
              aria-pressed={c === look.colour}
              onClick={() => onChange({ colour: c.key })}
            />
          ))}
        </span>
      </fieldset>
    </>
  );
}

/** One choice of a shape or eyes: drawn alone in the box. */
function Pick(props: { part: string; name: string; on: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      className="pk"
      data-part={props.part}
      aria-label={props.name}
      aria-pressed={props.on}
      onClick={props.onClick}
    >
      <svg viewBox="0 0 48 48" aria-hidden="true">
        <g fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round">
          {props.children}
        </g>
      </svg>
    </button>
  );
}
