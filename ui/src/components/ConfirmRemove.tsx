import { useId } from 'react';
import './confirm.css';

/** A short question, Cancel and the verb, in place of the actions it asks about. Escape cancels it, not Settings. */
export function ConfirmRemove({
  name,
  onCancel,
  onRemove,
}: {
  name: string;
  onCancel: () => void;
  onRemove: () => void;
}) {
  const question = useId();
  return (
    <fieldset
      className="confirm"
      aria-labelledby={question}
      onKeyDown={(e) => {
        if (e.key !== 'Escape') return;
        e.preventDefault();
        e.stopPropagation();
        onCancel();
      }}
    >
      <span id={question}>Remove {name}?</span>
      {/* biome-ignore lint/a11y/noAutofocus: the question opens from Remove; Cancel is the safe answer */}
      <button type="button" autoFocus onClick={onCancel}>
        Cancel
      </button>
      <button type="button" className="danger" onClick={onRemove}>
        Remove
      </button>
    </fieldset>
  );
}
