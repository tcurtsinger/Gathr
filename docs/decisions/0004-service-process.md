# 0004. One service process, `gathrd`, with an HTTP and WebSocket API on localhost

Status: proposed. Date: 2026-11-06.

## Choice

- **One daemon, `gathrd`:** a single Rust binary on tokio that owns the database (0003), the engines (0001, 0002), scheduling and coordination (0010), the event record (0011) and the MCP server (0005). One per user: a second start finds the first through an exclusive lock on `<data>/gathrd.lock` and exits.
- **API:** [axum](https://crates.io/crates/axum) 0.8 with its `ws` feature. Plain HTTP (JSON) for reads and commands; one WebSocket per client for everything live (streamed tokens, events, task changes). The built UI is served from the same port (0012).
- **Where it listens:** `127.0.0.1` on a port chosen at first start and kept, written with a random token to `<data>/service.json`, readable only by the user.
- **Who may call it:** every request needs the token. Requests whose `Host` isn't `127.0.0.1` or `localhost` get 403; requests with an `Origin` other than the service's own or the Tauri window's get 403, WebSocket upgrades included. While remote use is on, the computer's Tailscale name and its HTTPS origin are accepted too, with a device token ([0016](0016-remote-use.md)). The window's origin per system is in [0015](0015-macos.md).
- **How the window starts it:** the Tauri app reads `service.json` and calls `/health`. If nothing answers, it starts `gathrd` itself as an ordinary child with `std::process::Command` (on Windows `CREATE_NO_WINDOW`, `CREATE_NEW_PROCESS_GROUP`; on macOS see [0015](0015-macos.md)), not through the shell plugin, and waits for `/health`. The binary still ships inside the installer through `bundle.externalBin`. `gathrd` keeps running when the window closes; Quit in the tray stops it gracefully.
- **Graceful stop:** on Quit or shutdown, the service stops taking work, interrupts running turns (0001), closes engine stdin, waits, then exits. Engines left over after a crash are found from the event record at next start and closed the same way, never killed by name.

## Why

- **The window can come and go.** Tauri's shell plugin kills every child it spawned when the app exits ([shell plugin source](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/shell/src/lib.rs); [sidecar docs](https://v2.tauri.app/develop/sidecar/)), so a sidecar started that way would take the engines down with the window, mid-turn and possibly mid-renewal (0002). Starting it ourselves keeps the bundling and drops the coupling. 0012 already puts engines outside the window for this reason.
- **One API for every client.** The window, a browser and scripts use the same HTTP and WebSocket API, as 0012 requires.
- **axum** is the most used Rust HTTP framework, MIT, with WebSockets and server-sent events built in, and rmcp's MCP server mounts on an axum router ([axum 0.8.9](https://crates.io/crates/axum), [rmcp](https://github.com/modelcontextprotocol/rust-sdk)), so one server serves the UI, the API and MCP.
- **Localhost is not private.** Any web page in the person's browser can send requests to `127.0.0.1`, and DNS rebinding can make a hostile page look same-origin. Jupyter answers non-local `Host` headers with 403 for this reason ([Jupyter Server config](https://jupyter-server.readthedocs.io/en/latest/other/full-config.html)); the MCP spec requires servers to validate `Origin` and recommends binding to `127.0.0.1` and authenticating ([MCP transports](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)). WebSockets are not covered by CORS, so the server must check `Origin` itself ([RFC 6455 §4.2.2](https://www.rfc-editor.org/rfc/rfc6455#section-4.2.2)). The Tauri window's origin on Windows is `http://tauri.localhost` ([Tauri 2.1.0 notes](https://v2.tauri.app/release/tauri/v2.1.0/)).
- **No console windows.** `gathrd` is built with `#![windows_subsystem = "windows"]` ([Rust reference](https://doc.rust-lang.org/reference/runtime.html#the-windows_subsystem-attribute)), and every child it starts gets `CREATE_NO_WINDOW` ([process creation flags](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags)). `DETACHED_PROCESS` is never used: it cancels `CREATE_NO_WINDOW` (same page), and the previous app saw child windows reappear with it.

## Rules that follow

- Every endpoint is reachable from tests without the window: the first slice's checks start `gathrd` on a temp data folder and a free port.
- The token never appears in a URL or a log. The window passes it in a header; a browser gets it once through a one-time link the window opens, exchanged for an HTTP-only cookie (details in the first slice).
- `service.json` and `gathrd.lock` are the only files the window reads directly.
- Diagnostics from day one: `gathrd` writes its own start, stop, crash and every child start and exit to the event record (0011), with the reason.

## Turned down

- **The service inside the Tauri process.** Simplest to start, but a window crash or reload ends every engine; 0012 turned it down for the same reason.
- **A Tauri sidecar spawned through the shell plugin.** It dies with the window.
- **A Windows service (SCM).** Runs as another account, outside the person's session and their engines' login folders, and needs admin to install.
- **A Job Object with kill-on-close for engines.** It would clean up after a crash, but by force-killing every engine at once, which is the mid-renewal kill 0002 rules out. Engines exit on their own when their stdin closes.
- **gRPC or a custom socket protocol.** A browser can't use them without a proxy.
- **actix-web, warp, poem.** actix-web is a fine alternative but rmcp integrates with axum; warp and poem release slowly ([crates.io](https://crates.io/crates/poem)).

## Not verified

- That `claude -p` and `codex app-server` exit by themselves when their stdin closes after the service dies. Claude Code documents waiting up to 10 minutes for background tasks after stdin closes ([headless](https://code.claude.com/docs/en/headless#background-tasks-at-exit)); the first slice measures both.
