import { Plus } from 'lucide-react';
import { useId, useRef, useState } from 'react';
import { BACKGROUNDS, drawnOn } from '../components/backgrounds';
import { ColourPicker } from '../components/ColourPicker';

interface Props {
  /** #RRGGBB, or null until the person picks: a grey. */
  background: string | null;
  /** The last colour not among the twelve, while the form is open: + keeps it after a ready colour is pressed. */
  remembered: string | null;
  /** A colour picked; custom when it came from the picker and isn't one of the twelve. */
  onChange: (change: { background: string; custom?: string }) => void;
}

/** A colour that isn't one of the twelve: it shows on the +. */
export const isCustom = (hex: string | null | undefined): hex is string =>
  !!hex && !BACKGROUNDS.some(([, ready]) => ready === hex);

/**
 * Settings > Agents' Background (0074): twelve ready colours, then + for any colour, which opens the picker under
 * the row. A colour not among the twelve shows on the +, pressed. After a ready colour is pressed, the + still shows
 * the last such colour, not pressed, until the form's edits are dropped (Cancel, Save, another agent); pressing it
 * picks that colour again and opens the picker at it. With none, + only opens the picker.
 */
export function BackgroundField({ background, remembered, onChange }: Props) {
  const picker = useId();
  const row = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);
  const picked = isCustom(background);
  const custom = picked ? background : remembered;
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
            onClick={() => onChange({ background: hex })}
          />
        ))}
        <button
          type="button"
          className="swatch swatch-custom"
          style={custom ? { background: custom } : undefined}
          aria-label="Custom"
          aria-pressed={picked}
          aria-expanded={open}
          popoverTarget={picker}
          onClick={() => !picked && custom && onChange({ background: custom })}
        >
          {!custom && <Plus aria-hidden="true" />}
        </button>
      </span>
      <ColourPicker
        id={picker}
        label="Custom"
        anchor={row}
        value={drawnOn(background)}
        onChange={(hex) => onChange({ background: hex, ...(isCustom(hex) && { custom: hex }) })}
        onToggle={setOpen}
      />
    </fieldset>
  );
}
