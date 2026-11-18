import { type ReactNode, useEffect, useState } from 'react';
import { IconButton } from './components/IconButton';
import { desktop, type Ipc } from './desktop';
import { report } from './report';
import './topline.css';

/**
 * The window's top line (0070): the band of shell above the frame, which the app's grid puts there. The desktop
 * window has no system title bar, so the line moves it: a press on any part that isn't a control drags, a double
 * click maximizes ("deep", Tauri's drag.js). On Windows it holds the window's three buttons at its right; the
 * Mac draws its own at its left, over the rail, where the line holds nothing. A browser gets neither.
 */
export function TopLine({ children }: { children?: ReactNode }) {
  const ipc = desktop();
  // The webview's user agent names its system: WebView2's says Windows, WKWebView's the Mac.
  const buttons = ipc && navigator.userAgent.includes('Windows');
  return (
    <header
      className={buttons ? 'topline with-buttons' : 'topline'}
      data-tauri-drag-region={ipc ? 'deep' : undefined}
    >
      {children}
      {buttons && <WindowButtons ipc={ipc} />}
    </header>
  );
}

function WindowButtons({ ipc }: { ipc: Ipc }) {
  const maximized = useMaximized(ipc);
  const run = (command: string) => () => send(ipc, command);
  return (
    <div className="window-buttons">
      <IconButton aria-label="Minimize" className="window-button" onClick={run('minimize')}>
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0 5.5h10" />
        </svg>
      </IconButton>
      <IconButton
        aria-label={maximized ? 'Restore' : 'Maximize'}
        className="window-button"
        onClick={run('toggle_maximize')}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          {maximized ? (
            <>
              <rect x=".5" y="2.5" width="7" height="7" rx="1.5" />
              <path d="M2.5 2.5V2A1.5 1.5 0 0 1 4 .5h4A1.5 1.5 0 0 1 9.5 2v4A1.5 1.5 0 0 1 8 7.5h-.5" />
            </>
          ) : (
            <rect x=".5" y=".5" width="9" height="9" rx="1.5" />
          )}
        </svg>
      </IconButton>
      <IconButton aria-label="Close" className="window-button close" onClick={run('close')}>
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="m.5.5 9 9M9.5.5l-9 9" />
        </svg>
      </IconButton>
    </div>
  );
}

/** Whether the window is maximized, read again each time it resizes: maximizing, snapping and restoring all do. */
function useMaximized(ipc: Ipc) {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    const read = () => send(ipc, 'is_maximized').then((value) => setMaximized(value === true));
    read();
    addEventListener('resize', read);
    return () => removeEventListener('resize', read);
  }, [ipc]);
  return maximized;
}

/** A window command; one the window refuses goes to the event record, as the title bar's Theme does. */
function send(ipc: Ipc, command: string) {
  return ipc
    .invoke(`plugin:window|${command}`, {})
    .catch((e) => report({ name: 'gathr.ui.window.frame', error: command, attrs: { message: String(e) } }));
}
