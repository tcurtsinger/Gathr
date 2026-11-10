/**
 * The desktop window's IPC, allowed only what crates/gathr-app/capabilities lists, and only for Gathr's page on
 * 127.0.0.1. None in a browser.
 */
export function desktop(): { invoke(cmd: string, args: object): Promise<unknown> } | undefined {
  return (window as { __TAURI_INTERNALS__?: ReturnType<typeof desktop> }).__TAURI_INTERNALS__;
}
