import './switch.css';

interface Props {
  on: boolean;
  /** The ids of its label and of what it applies to. */
  labelledBy: string;
  describedBy?: string;
  disabled?: boolean;
  onChange: (on: boolean) => void;
}

/** The one switch: a native button, so Space and Enter toggle it, with the switch role. Busy, it
 * ignores presses but keeps focus, which a disabled button would drop. */
export function Switch({ on, labelledBy, describedBy, disabled, onChange }: Props) {
  return (
    <button
      type="button"
      role="switch"
      className="switch"
      aria-checked={on}
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      aria-disabled={disabled}
      onClick={() => disabled || onChange(!on)}
    >
      <span />
    </button>
  );
}
