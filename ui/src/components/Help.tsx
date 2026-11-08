import { type ReactNode, useId, useRef, useState } from 'react';
import './help.css';

/** A setting's detail behind a small ? (approvals/v1): a popover, so the form doesn't move. */
export function Help({ label, children }: { label: string; children: ReactNode }) {
  const id = useId();
  const button = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);

  /** Under the ?, or above it when there's no room below, kept 20 px inside the window. */
  const place = (panel: HTMLElement) => {
    const at = button.current?.getBoundingClientRect();
    if (!at) return;
    const below = at.bottom + 6 + panel.offsetHeight + 20 <= innerHeight;
    panel.style.left = `${Math.max(20, Math.min(at.left, innerWidth - panel.offsetWidth - 20))}px`;
    panel.style.top = `${Math.max(20, below ? at.bottom + 6 : at.top - panel.offsetHeight - 6)}px`;
  };

  return (
    <>
      <button
        ref={button}
        type="button"
        className="help-button"
        aria-label={label}
        aria-expanded={open}
        popoverTarget={id}
      >
        <span aria-hidden="true">?</span>
      </button>
      <section
        id={id}
        popover="auto"
        className="help-panel"
        aria-label={label}
        onToggle={(e) => {
          const shown = e.newState === 'open';
          setOpen(shown);
          if (shown) place(e.currentTarget);
        }}
      >
        {children}
      </section>
    </>
  );
}
