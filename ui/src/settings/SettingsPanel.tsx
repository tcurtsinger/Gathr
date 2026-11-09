import { Dialog } from '../components/Dialog';
import { Agents } from './Agents';
import { Diagnostics } from './Diagnostics';
import { Engines } from './Engines';
import { General } from './General';
import './settings.css';

export type Page = 'general' | 'engines' | 'agents' | 'diagnostics';

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
  ['general', 'General'],
  ['engines', 'Engines'],
  ['agents', 'Agents'],
  ['diagnostics', 'Diagnostics'],
];

/** Settings, centred over the current screen (0023). The screen under it keeps its state. */
export function SettingsPanel({ target, onPage, onReference, onClose }: Props) {
  return (
    <Dialog
      open={target !== null}
      title="Settings"
      onClose={onClose}
      opener={target?.opener}
      className="settings-panel"
    >
      {target && (
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
            {target.page === 'general' ? (
              <General onReference={onReference} />
            ) : target.page === 'engines' ? (
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
      )}
    </Dialog>
  );
}
