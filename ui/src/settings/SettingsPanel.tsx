import { useEffect, useRef } from 'react';
import { Agents } from './Agents';
import { Diagnostics } from './Diagnostics';
import { Engines } from './Engines';
import './settings.css';

export type Page = 'engines' | 'agents' | 'diagnostics';

export interface SettingsTarget {
  page: Page;
  /** Agents opens at this agent. */
  agent?: string;
  /** Diagnostics opens at this chat, and this turn when one is given. */
  chat?: string;
  title?: string;
  turn?: string;
  /** Diagnostics shows this reference's events, for a failure with no turn. */
  reference?: string;
  /** Gets focus back when Settings closes. */
  opener: HTMLElement | null;
}

interface Props {
  target: SettingsTarget | null;
  onPage: (page: Page) => void;
  /** Opens a failure's reference in Diagnostics. */
  onReference: (ref: string) => void;
  onClose: () => void;
}

const pages: [Page, string][] = [
  ['engines', 'Engines'],
  ['agents', 'Agents'],
  ['diagnostics', 'Diagnostics'],
];

/** Settings, centred over the current screen (0023). The screen under it keeps its state. */
export function SettingsPanel({ target, onPage, onReference, onClose }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const open = target !== null;
  const opener = useRef<HTMLElement | null>(null);
  if (target) opener.current = target.opener;

  useEffect(() => {
    const d = dialog.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    if (!open && d.open) {
      d.close();
      opener.current?.focus();
    }
  }, [open]);

  return (
    <dialog
      ref={dialog}
      className="settings-panel"
      aria-labelledby="settings-title"
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      {target && (
        <>
          <header className="settings-header">
            <h2 id="settings-title">Settings</h2>
            <button type="button" onClick={onClose}>
              Close
            </button>
          </header>
          <div className="settings-body">
            <nav className="settings-categories" aria-label="Settings">
              {pages.map(([page, label]) => (
                <button
                  type="button"
                  key={page}
                  aria-current={target.page === page ? 'page' : undefined}
                  onClick={() => onPage(page)}
                >
                  {label}
                </button>
              ))}
            </nav>
            <div className="settings-page">
              {target.page === 'engines' ? (
                <Engines onReference={onReference} />
              ) : target.page === 'agents' ? (
                <Agents start={target.agent} onReference={onReference} />
              ) : (
                <Diagnostics
                  chat={target.chat}
                  title={target.title}
                  turn={target.turn}
                  reference={target.reference}
                />
              )}
            </div>
          </div>
        </>
      )}
    </dialog>
  );
}
