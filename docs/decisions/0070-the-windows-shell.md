# 0070: The window's shell

Status: accepted (owner, 2026-11-17: the frame mock-up, "Yes, use it", A in both of its choices). Date: 2026-11-17.

Built: yes. Proof: `ui/src/Rail.tsx`, `ui/src/app.css`, `ui/src/narrow.test.tsx` (the widths), `ui/src/sidebar.test.tsx` (a row's More), `ui/src/update.test.tsx` (Update on the rail), `ui/src/TopLine.tsx` and `ui/src/TopLine.test.tsx` (the window's buttons and the band that moves it), `crates/gathr-app/src/main.rs` (`window` and `frame`). The look is proved by pictures; dragging, resizing and snapping the native window by the owner's hand check on Windows. Not verified: the Mac's window.

Amends [0068](0068-the-windows-colours.md), which left the shell's colour to this change.

## The choice

- **A shell around a frame.** The window's own surface, `--shell` (dark `#2c2c2c`, the lightest of the dark surfaces; light a grey darker than the frame), holds a rail at the left and one top line above. Inside it, one frame holds the sidebar, the chat and the panel: rounded 12px at every corner, ringed 1px in `--shell-edge`, with 6px of shell on its right and below.
- **The rail: 44px, icons only.** At the top, Home, a mark of where you are. The window has one place, so it is drawn as current and does nothing: no button that does nothing. Room is left under it for places to come. At the foot, Update (only while a release is ready; a turning loader while it installs) above the Settings cog. Both come from the sidebar's foot with their behaviour unchanged.
- **One top line, 40px.** From where the sidebar ends: a short divider on the sidebar's edge, the chat's title and its More, then at the right Summary's faces and the panel's button, open or closed. The chat has no header of its own, and the panel's tabs are a 38px row of their own at its top.
- **Widths.** The frame is the window less the rail and the margin. The panel takes 40% of the room beside the sidebar, 360 to 760px; when that would leave the chat under 360px, the sidebar gives way down to 190px and the panel is 360px. At 1440, 1200, 1000 and 960 wide, sidebar / chat / panel: 246 / 686 / 458, 246 / 542 / 362, 230 / 360 / 360 and 190 / 360 / 360.
- **The sidebar, tighter.** Rows 30px high, the word mark's block 36px with 8px above it, so its centre lines up with Home's. A chat's More shows only on hover or keyboard focus, the open chat's too; a project's row greys as one, its + and More inside it. Search shows no key words (Ctrl K still opens it). A chat's dot sits after its cut title.
- **No system title bar** (the owner's ask, C72; its try checked by hand on Windows 11, 2026-11-18). The desktop window is built with no title bar of its own: on Windows with no decorations but the system's border, shadow and resizing edges (`decorations(false)`, `shadow(true)`); on the Mac with the title bar laid over the page and its title hidden (`TitleBarStyle::Overlay`, `hidden_title`), so the system's three buttons sit at the band's left, over the rail and the sidebar, where the top line holds nothing. The top line is the whole band above the frame and Tauri's drag region (`data-tauri-drag-region="deep"`): a press on any part of it that isn't a control moves the window, a double click maximizes or restores it. On Windows its right end holds Minimize, Maximize (Restore while maximized, read from the window at each resize) and Close, drawn as Windows draws them, at the window's own top right corner. A browser gets neither. A modal dialog makes everything outside it inert, the top line too, so the shared Dialog holds the same band (`WindowBand`): fixed over the window's top 40px, the same drag region and, on Windows, the same three buttons at the same pixels, out of the dialog's keyboard order; with Settings open the window still moves, minimizes, maximizes and closes, as it did under the system's title bar. The page's IPC may only drag, minimize, maximize, close and read whether the window is maximized (`capabilities/window.json`). "Start failed", a file that draws no top line, gets the system's title bar back (`frame` in `main.rs`) until the window loads Gathr's page again.
- **Which system the page is on** comes from the webview's user agent: WebView2's names Windows, WKWebView's the Mac. Nothing else in the page needs it, and the window's IPC has no command for it without the OS plugin.

## Why

The owner's ask (2026-11-17): a curved shell around the menu, a thin bar at the left with Home now and the settings cog and update icon at its foot, and room there for what comes later. One top line ends the two bars of header the chat and the panel each had.

## Turned down

- **B for the frame's edges:** the frame running to the window's right and bottom edges, only its top-left corner rounded. A 6px margin with every corner rounded reads as one shape on the shell.
- **B for the sidebar's tint:** the sidebar on the chat's background with a hairline between them. The sidebar on the frame tint, the chat on the background, keeps the three steps of 0068.
- **Home as a button now.** With one place, it would do nothing.
