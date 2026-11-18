import { Plus } from 'lucide-react';
import { useId, useRef, useState } from 'react';
import { ColourPicker } from '../components/ColourPicker';
import { BACKGROUNDS, drawnOn } from '../faces/backgrounds';

interface Props {
  /** #RRGGBB, or null until the person picks: a grey. */
  background: string | null;
  onChange: (background: string) => void;
}

/**
 * Settings > Agents' Background (0074): twelve ready colours, then + for any colour, which opens the picker under
 * the row. A colour not among the twelve shows on the +, pressed.
 */
export function BackgroundField({ background, onChange }: Props) {
  const picker = useId();
  const row = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);
  const custom =
    background !== null && !BACKGROUNDS.some(([, hex]) => hex === background) ? background : null;
  return (
    <fieldset className="agent-field">
      <legend>Background</legend>
      <span className="swatches" ref={row}>
        {BACKGROUNDS.map(([name, hex]) => (
          <button
            type="button"
            key={hex}
            className="swatch"
            style={{ background: hex }}
            aria-label={name}
            aria-pressed={background === hex}
            onClick={() => onChange(hex)}
          />
        ))}
        <button
          type="button"
          className="swatch swatch-custom"
          style={custom ? { background: custom } : undefined}
          aria-label="Custom"
          aria-pressed={custom !== null}
          aria-expanded={open}
          popoverTarget={picker}
        >
          {!custom && <Plus aria-hidden="true" />}
        </button>
      </span>
      <ColourPicker
        id={picker}
        label="Custom"
        anchor={row}
        value={drawnOn(background)}
        onChange={onChange}
        onToggle={setOpen}
      />
    </fieldset>
  );
}
