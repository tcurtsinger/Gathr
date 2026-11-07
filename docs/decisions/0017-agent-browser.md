# 0017. The agents' browser: a browser the service runs, shown as a live stream

Status: proposed. Date: 2026-11-07.

Agents need a real browser: open a page, read it, click and type, and keep their own sign-ins. The person needs to watch it and take over (to sign in, or to help). Gathr's UI is a web page shown in a Tauri window, and also in a browser tab (0012) and from another computer (0016), so where the agents' browser lives decides where it can be seen.

## Choice

- **The service runs the browser, not the window.** `gathrd` starts a Chromium browser per agent account that uses one: Microsoft Edge on Windows (always installed), Chrome or Edge on macOS, whichever is there. Headless, so no window appears on the desktop.
- **Driven over a pipe, never a port.** The browser starts with `--remote-debugging-pipe`, and the service speaks the Chrome DevTools Protocol (CDP) over it. No other program on the computer can drive it, as no debugging port is opened.
- **One profile folder per agent**, in Gathr's data folder (`<data>/browsers/<agent>`), holding that agent's cookies, sign-ins and storage. Never the person's own Chrome or Edge profile, and nothing is copied from it.
- **Every client sees it as a live stream.** The service relays CDP's screencast (JPEG frames) to the UI over the WebSocket it already has (0004), and sends the person's mouse, wheel and keys back as CDP input events. The panel is one React component, the same in the window, in a browser tab and on the other computer.
- **A real window when the stream isn't enough.** **Open window** restarts the agent's browser with a visible window on the same profile, for the things a stream can't carry: passkeys and security keys, file pickers, OS dialogs. Closing it returns to headless. This only works on the computer that runs the agent.
- **Agents use it through Gathr's MCP server (0005):** open, read, click, type, screenshot, tabs. The tools take an agent's own browser only; a tool call never reaches another agent's profile.
- **Diagnostics from the first attempt.** Starts, stops, crashes, each tool call and its timing go to the event record (0011).

## Why

- **One browser for every place Gathr is used.** Gathr is used from more than one computer (0016) and Gathr opens in a browser tab (0012). A stream from the service shows in all three. A browser view inside the Tauri window shows only in that window, on that computer.
- **A browser tab can't hold the page itself.** Most sites refuse to be framed with `X-Frame-Options` or CSP `frame-ancestors`. In the test below, an iframe of a page sending `X-Frame-Options: DENY` loaded with its content blocked. So the browser-tab and remote cases need a stream in any design. The only question is whether the window gets a second, native path, and two paths cost twice the work and the testing.
- **The Mac's WKWebView can't be driven like this.** It has no CDP. Apple's `isInspectable` lets Safari's Web Inspector attach for debugging by hand ([WKWebView.isInspectable](https://developer.apple.com/documentation/webkit/wkwebview/isinspectable), [WebKit blog](https://webkit.org/blog/13936)), and Tauri's own docs say WebDriver testing doesn't work on macOS "due to macOS not having a WKWebView driver tool available" ([Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/)). A native view on the Mac would be driven only by injected JavaScript: no trusted input, no network events, no cross-origin frames. A Chromium browser behaves the same on both systems.
- **Tauri's views inside a window are still unstable.** Adding a second webview to a window (`Window::add_child`) needs Tauri's `unstable` feature ([webview API](https://v2.tauri.app/reference/javascript/api/namespacewebview/), [#9059](https://github.com/tauri-apps/tauri/pull/9059)). Per-view data folders exist, but the Mac form needs macOS 14 ([`data_store_identifier`](https://docs.rs/tauri-runtime/latest/tauri_runtime/webview/struct.WebviewAttributes.html)), and on Windows each data folder starts its own group of WebView2 processes anyway ([WebView2 process model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model)). So a browser per agent costs about the same in either design.
- **The window can close without stopping the agent's browsing.** 0012 keeps engines out of the window so a closed or crashed window never touches a running turn. A browser inside the window would break that: an agent browsing while the window is in the tray, or while the person works from another computer, would lose its pages.
- **No port, by Chrome's own rules too.** Since Chrome 136, `--remote-debugging-port` is ignored on the default profile, and Google recommends "a custom user data dir… to isolate any debugging from any real profiles" ([Chrome blog](https://developer.chrome.com/blog/remote-debugging-port)). A profile per agent with a pipe meets that, and a pipe also stops other local programs from attaching. The previous app reached the same rule by driving its pages through Electron's in-process debugger, with no port.
- **Measured on this PC** (Edge 156, headless, `--disable-gpu`, `--remote-debugging-pipe`, a local 500-row page, 2026-11-07):

  | | |
  |---|---|
  | Browser ready after start | 228 ms |
  | Page load | 25 ms |
  | First screencast frame | 15 ms |
  | Wheel scroll to new frame | median 7 ms, max 17 ms (15 scrolls) |
  | Key press to new frame | median 17 ms (18 keys; the typed text arrived intact) |
  | Frames during continuous scrolling | 58 per second |
  | Agent A's sign-in cookie after a browser restart | kept |
  | Agent B, own profile, same site | no cookie |
  | Processes per browser | 17, about 870 MB working set (summed, so shared memory counts more than once) |

  These are CDP times inside one computer. The relay through the service to the UI, and over Tailscale to another computer, adds to them; measured when built.

## Turned down

- **A WebView2 or WKWebView view inside the Tauri window.** The best feel on Windows (a real page, native input, CDP in-process through `CallDevToolsProtocolMethod`, no port), and what the previous app did with Electron. Turned down: it can't be shown in a browser tab or on another computer, the Mac's WKWebView can't be driven, it needs Tauri's unstable multi-webview feature, and it ties the agent's browsing to the window being open.
- **Both: the native view in the window, the stream elsewhere.** Two browser paths to build, test and keep in step, with different behaviour on each system, for a smoother panel on Windows only. Reconsidered if the stream proves too rough in use.
- **The person's own Chrome or Edge profile.** Chrome 136 and later refuse remote debugging on it, and an agent would act with every sign-in the person has.
- **A debugging port instead of a pipe.** Any program on the computer could attach to the agents' signed-in browsers.
- **Shipping a Chromium build with Gathr** (Chrome for Testing, CEF). A whole browser for Gathr to download and keep patched, when Edge is on every Windows PC. On a Mac with neither Chrome nor Edge, the browser tools say the browser is missing, and Settings links to the download; nothing installs without the person.
- **Playwright or Puppeteer as the driver.** Both are Node, which the service doesn't have; the CDP calls the tools need are a small, stable set the service can send itself.

## Rules that follow

- The browser panel is a shared component: the same stream, input and **Open window** wherever Gathr is shown.
- Gathr never opens a debugging port, and never points a browser at a profile outside `<data>/browsers`.
- The browser panel streams only while someone is looking at it; agents' tool calls work with no viewer.
- Each agent's profile is removed with the agent, after a confirm.

## Not verified

- `--remote-debugging-pipe` with Chrome and Edge on macOS (the pipe works on Windows, above).
- That a profile used headless opens in **Open window** with its sign-ins intact, and back.
- Whether sites that block automated browsers (Google sign-in, for one) accept the headless browser, or need **Open window** to sign in.
- Typing in languages that use an input method (Japanese, Chinese) through the stream.
- Stream latency over Tailscale between two computers.
