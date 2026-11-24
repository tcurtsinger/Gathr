import { useId } from 'react';
import { Button } from '@/components/ui/button';
import { danger } from './Confirm';
import './confirm.css';

/** A short question, Cancel and the verb, in place of the actions it asks about. Escape cancels it, not Settings. */
export function ConfirmRemove({
  name,
  verb = 'Remove',
  body,
  waiting,
  onCancel,
  onRemove,
}: {
  name: string;
  verb?: string;
  /** After the question: what it will do, muted. */
  body?: string;
  /** The verb stays off until what it will do is known. */
  waiting?: boolean;
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
      <span id={question}>
        {verb} {name}?{body && <span className="confirm-line"> {body}</span>}
      </span>
      <Button type="button" variant="outline" autoFocus onClick={onCancel}>
        Cancel
      </Button>
      <Button type="button" variant="outline" className={danger} disabled={waiting} onClick={onRemove}>
        {verb}
      </Button>
    </fieldset>
  );
}
