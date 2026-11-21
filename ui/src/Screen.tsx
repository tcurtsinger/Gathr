import type { ReactNode } from 'react';
import { ResizeHandle } from './components/ResizeHandle';
import { TopLine } from './TopLine';
import './screen.css';

interface Props {
  /** Its navigation's title, and both panes' name for screen readers. */
  name: string;
  /** Beside the title. */
  action?: ReactNode;
  /** The navigation under the title. */
  nav: ReactNode;
  /** The main panel's page. */
  children: ReactNode;
}

/** A screen in place of the chat list and the chat, as Plugins and Agents are: its own navigation, then a page. */
export function Screen({ name, action, nav, children }: Props) {
  return (
    <>
      <nav className="sidebar screen-nav" aria-label={name}>
        <div className="brand">
          <strong>{name}</strong>
          {action}
        </div>
        {nav}
        <ResizeHandle
          label="Resize navigation"
          side="right"
          name="--sidebar-width"
          min={190}
          max={() => 400}
        />
      </nav>
      <TopLine />
      <main className="chat screen" aria-label={name}>
        <div className="screen-page">{children}</div>
      </main>
    </>
  );
}
