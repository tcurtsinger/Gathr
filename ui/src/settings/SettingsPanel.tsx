import { Activity, Cpu, Flag, Folder, type LucideIcon, SlidersHorizontal, Users } from 'lucide-react';
import { useState } from 'react';
import { Dialog } from '../components/Dialog';
import { Diagnostics } from './Diagnostics';
import { Engines } from './Engines';
import { Friction } from './Friction';
import { General, Teamwork } from './General';
import { Projects } from './Projects';
import './settings.css';

export type Page = 'general' | 'engines' | 'teamwork' | 'projects' | 'turns' | 'friction';

export interface SettingsTarget {
  page: Page;
  /** Projects opens at this project's page. */
  project?: string;
  /** Turns opens at this chat, and this turn when one is given. */
  chat?: string;
  title?: string;
  turn?: string;
  /** Turns shows this reference's events, for a failure with no turn. */
  reference?: string;
  /** Gets focus back when Settings closes. */
  opener: HTMLElement | null;
}

interface Props {
  target: SettingsTarget | null;
  onPage: (page: Page) => void;
  /** Opens a failure's reference in Turns. */
  onReference: (ref: string) => void;
  onClose: () => void;
}

const groups: [string, [Page, string, LucideIcon][]][] = [
  [
    'App',
    [
      ['general', 'General', SlidersHorizontal],
      ['engines', 'Engines', Cpu],
    ],
  ],
  [
    'Team',
    [
      ['teamwork', 'Teamwork', Users],
      ['projects', 'Projects', Folder],
    ],
  ],
  [
    'Diagnostics',
    [
      ['turns', 'Turns', Activity],
      ['friction', 'Friction', Flag],
    ],
  ],
];

/** Settings, centred over the current screen (0023): its pages at the left in three groups. The screen under it keeps its state. */
export function SettingsPanel({ target, onPage, onReference, onClose }: Props) {
  // Each pick in the left column opens its page afresh: Projects again goes back to the list.
  const [visit, setVisit] = useState(0);
  return (
    <Dialog
      open={target !== null}
      title="Settings"
      onClose={onClose}
      opener={target?.opener}
      className="settings-panel"
      closeIcon
      noHeader
    >
      {target && (
        <div className="settings-body">
          <div className="settings-side">
            <h2>Settings</h2>
            <nav aria-label="Settings">
              {groups.map(([group, pages]) => (
                <div key={group} className="settings-pages">
                  <p>{group}</p>
                  {pages.map(([page, label, Icon]) => (
                    <button
                      type="button"
                      key={page}
                      aria-current={target.page === page ? 'page' : undefined}
                      onClick={() => {
                        setVisit((n) => n + 1);
                        onPage(page);
                      }}
                    >
                      <Icon aria-hidden="true" />
                      {label}
                    </button>
                  ))}
                </div>
              ))}
            </nav>
          </div>
          <div className="settings-page" key={visit}>
            {target.page === 'general' ? (
              <General onReference={onReference} />
            ) : target.page === 'engines' ? (
              <Engines onReference={onReference} />
            ) : target.page === 'teamwork' ? (
              <Teamwork onReference={onReference} />
            ) : target.page === 'projects' ? (
              <Projects start={target.project} onReference={onReference} />
            ) : target.page === 'friction' ? (
              <Friction />
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
