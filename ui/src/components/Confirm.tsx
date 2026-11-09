import { useState } from 'react';
import { Button } from '@/components/ui/button';
import type { Failure } from '../api';
import { Dialog } from './Dialog';
import { Notice } from './Notice';
import './confirm.css';

interface Props {
  /** The short question: "Delete Launch checklist?". */
  question: string;
  verb: string;
  danger?: boolean;
  onCancel: () => void;
  /** Settles when the service has done it; a failure keeps the question open with its reference. */
  onConfirm: () => Promise<unknown>;
  onReference: (ref: string) => void;
}

/** A question, Cancel and the verb, over the current screen (a variant of the one Dialog). Cancel has focus. */
export function Confirm({ question, verb, danger, onCancel, onConfirm, onReference }: Props) {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const go = () => {
    setBusy(true);
    setFailure(null);
    onConfirm().catch((f: Failure) => {
      setBusy(false);
      setFailure(f);
    });
  };
  return (
    <Dialog open bare title={question} className="confirm-dialog" onClose={() => busy || onCancel()}>
      {failure && (
        <Notice
          danger
          title={failure.error}
          reference={failure.ref ? { ref: failure.ref, open: onReference } : undefined}
          links={failure.why && <span className="muted">{failure.why}</span>}
        />
      )}
      <footer>
        <span className="visually-hidden" role="status">
          {busy ? 'Saving' : ''}
        </span>
        <Button size="sm" variant="outline" autoFocus disabled={busy} onClick={onCancel}>
          Cancel
        </Button>
        <Button size="sm" variant={danger ? 'destructive' : 'default'} disabled={busy} onClick={go}>
          {verb}
        </Button>
      </footer>
    </Dialog>
  );
}
