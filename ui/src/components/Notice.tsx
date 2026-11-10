import type { ReactNode } from 'react';
import type { Failure } from '../api';
import './notice.css';

interface Props {
  title: string;
  danger?: boolean;
  /** An error's reference: opens it in Diagnostics. */
  reference?: { ref: string; open: (ref: string) => void };
  /** Links shown after the title and reference. */
  links?: ReactNode;
  /** The action at the right. */
  action?: ReactNode;
}

/** The one notice: a reply that stopped, waits or failed, shown where the reply belongs. */
export function Notice({ title, danger, reference, links, action }: Props) {
  return (
    <div className={danger ? 'notice danger' : 'notice'} role={danger ? 'alert' : 'status'}>
      <span>{title}</span>
      {reference && (
        <button
          type="button"
          className="ref"
          onClick={() => reference.open(reference.ref)}
          aria-label={`Reference ${reference.ref}`}
        >
          {reference.ref}
        </button>
      )}
      {links}
      {action && <span className="notice-action">{action}</span>}
    </div>
  );
}

/** A request the service refused: what failed, why, and its reference (opened when it can be). */
export function FailureNotice({
  failure,
  onReference,
  links,
  action,
}: {
  failure: Pick<Failure, 'error' | 'why' | 'ref'>;
  onReference?: (ref: string) => void;
  links?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <Notice
      danger
      title={failure.error}
      reference={failure.ref && onReference ? { ref: failure.ref, open: onReference } : undefined}
      links={
        <>
          {failure.ref && !onReference && <code className="ref">{failure.ref}</code>}
          {failure.why && <span className="muted">{failure.why}</span>}
          {links}
        </>
      }
      action={action}
    />
  );
}
