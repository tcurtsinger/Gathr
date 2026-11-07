# 0012. React and TypeScript, in a Tauri window, also served to a browser

Status: proposed. Date: 2026-11-06. The framework is the owner's choice (React + TypeScript).

## Choice

- The UI is a React and TypeScript web app. It talks to the service only over the service's HTTP and WebSocket API, the same API any client uses.
- The service serves the built UI on localhost, so a browser on this machine can use Gathr.
- The desktop app is a Tauri 2 window that shows that UI and adds what a browser can't: a tray icon, notifications, starting with Windows, and the window itself.
- The window holds no state and owns no engines. Closing or reloading it never touches a running turn.

## Why

- **One app for the window and the browser.** If the UI called Tauri commands directly, a browser couldn't use it. With one API, the window is just another client.
- **The window can fail without harm.** Engines renew their logins in the background, and a process killed mid-renewal spends the login (lesson from the previous app). Keeping engines in the service, not in the window's process, means a crashed or reloaded window never kills an engine.
- **Small and native on Windows.** Tauri renders with Microsoft Edge WebView2 on Windows ([prerequisites](https://v2.tauri.app/start/prerequisites/#webview2)), which "is already installed on Windows 10 (from version 1803 onward) and later versions of Windows" (same page), so the app doesn't ship its own browser engine. Its installer downloads the WebView2 bootstrapper only if the runtime is missing, and can embed it instead ([Windows installer](https://v2.tauri.app/distribute/windows-installer/#webview2-installation-options)).
- **Tauri can carry the service.** Tauri bundles external binaries as "sidecars" through `bundle.externalBin` in `tauri.conf.json` ([sidecar](https://v2.tauri.app/develop/sidecar/)), so the service binary ships inside the same installer. How the window starts the service, or finds one already running, is decided with the service process (per 0001-0009, Cass).
- **React** (the owner's pick): agents write it most reliably, the chat components the owner wanted to reuse ([assistant-ui](https://github.com/assistant-ui/assistant-ui)) are React, and the owner's own React code from the previous app can be ported where it fits. No visual code from the previous app is reused: the look comes from the design picked in Phase 0.

## Rules that follow

- Every screen works in the browser as in the window. Window-only features (tray, notifications, start with Windows) degrade to nothing in a browser, never to an error.
- The browser path is localhost only by default, with a token the window holds and the service checks. Serving beyond this machine is a later decision.
- The UI reports its own timings (screen open, chat switch, first token shown) to the service as events (0011).
- Shared patterns (messages, links, notices, buttons, empty states) live in one component library built from the design system, per AGENTS.md.
- Build tooling and the component and state libraries are chosen at the first slice by the frontend engineer, as decision files of their own.

## Turned down

- **Electron.** It ships Chromium and Node with the app; Tauri uses the WebView2 runtime Windows already has, and the backend is Rust, where the service lives anyway. The previous app is Electron.
- **The service inside the Tauri process, with Tauri commands as the API.** One process is simpler to start, but a window crash then kills engines mid-turn and mid-renewal, and a browser can't reach the UI.
- **A UI in Rust (Leptos, Dioxus and the like).** Fewer ready chat components and less agent fluency than React; the owner picked React.
- **Solid or Svelte.** Offered with React; the owner picked React.
