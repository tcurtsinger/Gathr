import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import type { Failure } from '../api';
import { Dialog } from './Dialog';
import { FailureNotice } from './Notice';
import './confirm.css';

/** Outlined danger, not the registry's filled destructive: Delete and Remove (DESIGN.md "Archive"). */
export const danger = 'border-danger-border text-danger hover:bg-danger-surface';

interface Props {
  /** The short question: "Delete Launch checklist?". */
  question: string;
  /** Under the question, a sentence or two: what it does and what is kept. */
  body?: string;
  verb: string;
  danger?: boolean;
  /** The verb waits: what the body says hasn't arrived yet. */
  waiting?: boolean;
  /** What the body needed failed to load: shown with its reference, and the verb stays off. */
  loadFailure?: Failure | null;
  /** Gets focus back once the question goes, if it's still on the page. */
  opener?: HTMLElement | null;
  onCancel: () => void;
  /** Settles when the service has done it; a failure keeps the question open with its reference. */
  onConfirm: () => Promise<unknown>;
  onReference: (ref: string) => void;
}

/** A question, Cancel and the verb, over the current screen (a variant of the one Dialog). Cancel has focus. */
export function Confirm({
  question,
  body,
  verb,
  danger: isDanger,
  waiting,
  loadFailure,
  opener,
  onCancel,
  onConfirm,
  onReference,
}: Props) {
  const [busy, setBusy] = useState(false);
  useEffect(() => () => opener?.focus(), [opener]);
  const [failure, setFailure] = useState<Failure | null>(null);
  const go = () => {
    setBusy(true);
    setFailure(null);
    onConfirm().catch((f: Failure) => {
      setBusy(false);
      setFailure(f);
    });
  };
  const shown = failure ?? loadFailure;
  return (
    <Dialog open bare title={question} className="confirm-dialog" onClose={() => busy || onCancel()}>
      {body && <p className="confirm-body muted">{body}</p>}
      {shown && <FailureNotice failure={shown} onReference={onReference} />}
      <footer>
        <span className="visually-hidden" role="status">
          {busy ? 'Saving' : ''}
        </span>
        <Button type="button" size="sm" variant="outline" autoFocus disabled={busy} onClick={onCancel}>
          Cancel
        </Button>
        <Button
          type="button"
          size="sm"
          variant={isDanger ? 'outline' : 'default'}
          className={isDanger ? danger : undefined}
          disabled={busy || waiting || !!loadFailure}
          onClick={go}
        >
          {verb}
        </Button>
      </footer>
    </Dialog>
  );
}
