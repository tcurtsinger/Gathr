export interface Ipc {
  invoke(cmd: string, args: object): Promise<unknown>;
}

/**
 * The desktop window's IPC, allowed only what crates/gathr-app/capabilities lists, and only for Gathr's page on
 * 127.0.0.1. None in a browser.
 */
export function desktop(): Ipc | undefined {
  return (window as { __TAURI_INTERNALS__?: Ipc }).__TAURI_INTERNALS__;
}

/** The system's folder picker: the folder picked, or null when closed without one. Only in the desktop window. */
export function pickFolder(title: string, from: string | null): Promise<string | null> {
  const window = desktop();
  if (!window) return Promise.reject(new Error('No desktop window'));
  const options = { directory: true, defaultPath: from, title, canCreateDirectories: true };
  return window
    .invoke('plugin:dialog|open', { options })
    .then((picked) => (typeof picked === 'string' ? picked : null));
}
