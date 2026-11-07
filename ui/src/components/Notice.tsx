import type { ReactNode } from 'react';
import './notice.css';

interface Props {
  title: string;
  danger?: boolean;
  /** An error's reference: opens its turn in Diagnostics. */
  reference?: { ref: string; open: () => void };
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
          onClick={reference.open}
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
