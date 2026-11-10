import { useId } from 'react';
import { Button } from '@/components/ui/button';
import { danger } from './Confirm';
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
      <Button type="button" variant="outline" autoFocus onClick={onCancel}>
        Cancel
      </Button>
      <Button type="button" variant="outline" className={danger} onClick={onRemove}>
        Remove
      </Button>
    </fieldset>
  );
}
