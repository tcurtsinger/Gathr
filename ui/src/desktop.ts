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
