import { type ReactNode, useEffect, useId, useRef } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import './dialog.css';

interface Props {
  open: boolean;
  title: string;
  onClose: () => void;
  /** Where focus goes back to on close; by default, what had it when the dialog opened. */
  opener?: HTMLElement | null;
  className?: string;
  /** No Close in the header: a confirm, answered by its own buttons or Escape. */
  bare?: boolean;
  children: ReactNode;
}

/** Gathr's one modal: centred over the current screen, its title and Close on top. Escape closes it. */
export function Dialog({ open, title, onClose, opener, className, bare, children }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const back = useRef<HTMLElement | null>(null);
  const titled = useId();
  if (open && opener) back.current = opener;

  useEffect(() => {
    const d = dialog.current;
    if (!d) return;
    if (open && !d.open) {
      back.current ??= document.activeElement as HTMLElement;
      d.showModal();
    }
    if (!open && d.open) {
      d.close();
      back.current?.focus();
      back.current = null;
    }
  }, [open]);

  return (
    <dialog
      ref={dialog}
      className={cn('dialog', className)}
      aria-labelledby={titled}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      {open && (
        <>
          <header className="dialog-header">
            <h2 id={titled}>{title}</h2>
            {!bare && (
              <Button type="button" variant="ghost" onClick={onClose}>
                Close
              </Button>
            )}
          </header>
          {children}
        </>
      )}
    </dialog>
  );
}
