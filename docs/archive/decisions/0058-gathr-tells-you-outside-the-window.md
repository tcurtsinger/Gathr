# 0058: When a chat needs you, Gathr tells you outside the window

Status: accepted (owner, 2026-11-15: Stage 3's part B, with the answer "Yes, tell me again" to whether something new in a chat that already needs you is told). Date: 2026-11-15.

Built: yes. Proof: `crates/gathr/src/notice.rs` (its tests), `crates/gathr/tests/chat_state.rs`, `crates/gathr/tests/waiting.rs`, `ui/src/open-chat.test.tsx`. The system showing a notification and a click on it are checked by hand on installed builds, on Windows and on macOS ([window.md](../window.md)).

Builds on [0010](0010-coordination-as-state.md), [0012](0012-ui-shell.md) and [0050](0050-what-a-chat-waits-on.md). Serves promise 6 of [the charter](../charter.md#the-promises), part 6.8 of [the truth map](../truth-map.md): the person is told when a chat needs them, without opening Gathr. Stage 3's part B.

## The choice

- **B0: the app reads the service every 5 seconds.** A thread in the desktop app (`crates/gathr-app/src/notify.rs`) reads `GET /api/chats` (each chat's `needs`) and `GET /api/pauses` (whether the whole app is paused). It runs only in the installed app: the dev window has no service of its own and shows nothing.
  - A tick counts only when both routes answer 200. Anything else, a refused connection or a changed token in a hand-over included, keeps the last state, so a failed read never looks like a resume or a start.
  - It does nothing while the service's info is missing (Quit, or Start failed showing) or while an update installs.
  - For a chat that needs the person it reads `GET /api/chats/<id>/waiting`: at the first read, while the window is in front, and while the chat isn't told. A told chat isn't read until the window has been in front. During an app pause, when a new approval has no sign-in beside it, it also reads `GET /api/chats/<id>` for the approvals' reasons.
  - Nothing is written to disk. The state is in memory and is gone at Quit.
- **B1: when.** Something is new in a chat that needs the person when the chat starts needing them, or when its waiting lines have a sign-in, an approval or a question whose key isn't one the app saw at the last read. In front means the window is visible, not minimized and focused; if the app can't tell, it counts as not in front.
  - **A visit between reads counts too.** The app notes each time the window gains focus (tauri's `WindowEvent::Focused(true)`; a click on a notification focuses it through `reopen`), and the next read takes the note, so a visit shorter than 5 seconds isn't missed. A read that fails keeps the note for the next.
  - **With the window in front nothing is shown.** What is new is recorded as skipped, then everything now waiting counts as seen and every chat's mark clears, so the next new thing in any chat is told.
  - **With a visit since the last read and the window not in front now,** every chat's marks clear (told and held), and a told chat that left `needs` is let go, but nothing counts as seen: what waits is judged against what the app saw at the chat's last read, so something that came after the person left is told. A told chat wasn't read while told, so that is what waited when it was told. At the first read after a start there are no marks, so a visit changes nothing there.
- **B2: at most one notification per chat until the window has been in front.** A chat that was shown, or counted in a summary, is told. Nothing more is shown for it, and it isn't read, whatever comes up in it. A told chat that leaves `needs` and comes back before the window has been in front isn't told again; it is recorded as skipped, `told`.
- **B0.8: a line's key and the approval's time.** A line is known by its kind, task, `on`, agent and `since_us`, as the quiet rule keys it, whatever its words say. A question's `task` is its own number, and a sign-in's `since_us` is its turn's creation. An approval's was its turn's creation too, so a second approval asked right after the first was answered would have had the first one's key.
  - So the service keeps, for each turn with an approval waiting, when its approvals began waiting (`chat_state.rs`: `asking` holds the time, kept while one stays open and new after an answer), and the approval's line in the waiting route carries that time as `since_us` (`waiting.rs`: `Live.approvals` maps a turn to it). The window's list shows an approval's age from it, so the age counts from when it was asked.
  - A second approval asked while the first is still open in the same turn has the first one's time, so it isn't new. The person has one waiting from that agent already.
- **B3: an app-wide pause holds them.** A chat with something new during an app pause is held and recorded once, except when its new lines include a sign-in, or an approval Gathr itself asked for.
  - **Gathr's own ask** is an approval whose reason starts with `gathr::approvals::GATHR_ASKS`, "Gathr asks because". It is matched to its line by agent. The service's hook and its push check both build their reason from that constant, so the text is unchanged and the app and the service share one string. It covers writes outside the agent's folders or into Gathr's data folder, risky push commands, a wait's check on Claude Code, and pushes on either engine. Not covered, so held: the service's own check approvals ("Gathr runs …"), Codex's own sandbox asks and Claude Code's own asks in Ask mode.
  - **This relies on Claude Code passing the hook's reason back word for word** as the approval's reason. The only evidence is a test line made by hand in `claude/frames.rs`. If it rewords or drops the reason, the pause holds Gathr's own asks from Claude Code, and no test fails.
  - **Resume:** with the window not in front, every held chat is counted in one summary, "Resumed", "3 chats need you". With one, the summary carries that chat. A chat already shown during the pause is told, so it isn't counted again. A chat that needed the person before the pause, with nothing new in it, isn't held, and a held chat the person has visited the window since isn't counted. Only a pause of the whole app is read: a chat's or an agent's pause holds nothing.
- **B0.7: the first read after the app starts.**
  - After a restart for an update made while the window was hidden, it is a baseline: what waits now counts as seen, and nothing is shown.
  - After any other start, with chats that need the person and the window not in front, one notification: the chat's own for one chat, or "Waiting on you", "2 chats need you" for several. With the window in front, nothing.
- **B4: what it says.** The title is the chat's title. The body is the service's own words for the first new line that makes a chat need the person (a sign-in, an approval or a question), for example "Made-up Lead needs your approval", then "And 2 more" for the others of those kinds. During a pause it is the line the pause let through. With none of those, the first line nobody is on; with no lines, or a failed read, "Needs you". No sound, no action buttons and no icon of its own.
  - Other lines wait on the person without making a chat need them (a turn queued by a pause, a PR waiting on an answer), so the body never takes the first line blindly.
- **B5: a click brings the window forward on that chat.** The plugin `tauri-plugin-notification` 2.8.1 is added to the app and used from Rust only: no capability and no `build.rs` change. A handler is registered in `setup`, before any notification, because the plugin reports a click only for a notification shown while a handler exists. Each notification has a number; the app keeps the chat it was for, the last 50 only, and removes it at its click. A click on one no longer kept brings the window forward with no chat, as a summary's does, and is recorded with `chat` null. The click does what the tray's Open does (`reopen`), then runs `window.gathrOpenChat(id)` in the page. The page (`ui/src/App.tsx`) opens that chat as a click in the sidebar does. A chat that is gone is recorded and changes nothing.
  - If the service had stopped, `reopen` loads a new page first, and the window can open on its first chat. Accepted.
- **B6: every outcome is a `gathr.ui.notify` event**, through the window's `POST /api/ui/events`: `shown` (the chat, the notification's number, `rose`: whether the chat started needing the person; during a pause `why`: `sign_in` or `gathr_ask`), a summary (`summary`, `at` `start` or `resume`), `held`, `skipped` (`front` or `told`), `clicked`, and the page's error `gone`. A notification the plugin refuses is an error of type `show` with its message. Found with `gathr diag query "SELECT time, status, attrs FROM event_log WHERE name = 'gathr.ui.notify' ORDER BY ts"`, and `gathr diag failures` for an error.
  - `shown` says Gathr asked for the notification, not that the system showed it. With a click handler registered, the plugin builds the notification and hands it to a thread of its own, which drops the system's own error. So on Windows the error `show` can come only from building it, never from the system refusing it.
- **The decision is a pure function in the library**, `gathr::notice::decide`, so a service test feeds it the real routes' JSON. The thread, the window checks and the plugin calls are glue in the app, with no test.

## Changed from the spec

- **No `chat_opened` command.** The spec cleared a chat's mark when it was opened in the window. Here the mark clears once the window has been in front, at a read or, if only for a moment, between two. The window in front already marks every chat that needs the person in its list, and a `chat_opened` command would need an app manifest in `build.rs` and a capability, because the page is a remote page. What it drops: a chat the person saw in the list but didn't open counts as seen.
- **Something new in a chat that already needs the person is told,** at most once per chat until the window has been in front. The spec told only a chat that starts needing the person, which in a room with one team chat that nearly always has an open question would almost never tell an approval or a sign-in.
- **A told chat that leaves `needs` and comes back before the window has been in front isn't told again.** The spec showed it again.
- **An approval's line in the waiting route carries the time it was asked,** not its turn's.
- **The body's source is the service's waiting lines,** not the foot of the chat, which the window builds.
- **After a restart, one summary** (or the one chat's notification) rather than "each chat can be shown once more", and nothing after a restart for an update made while hidden.
- **Scenario 6j:** "none a second time before the chat is opened" is "none a second time before the window has been in front".
- **The scenario runs on Claude Code's recording only,** not once for each engine. The decision is engine-independent: `needs`, the waiting route and the approvals route don't depend on the engine, and the only Gathr ask on Codex, the push, is made by the service. No Codex approval recording exists.
- **The Mac is checked by hand after merge,** on the owner's installed Gathr, as Windows is.

## Why

- With the window away, a chat that waits on the person stops its agents until they look. Telling them is the point of Gathr knowing what a chat waits on.
- Once per chat until the person has looked keeps a burst in one chat to one notification, and tells the next new thing after they have.
- The window reads every 5 seconds from a thread of its own, not inside the follow loop: a refused connection takes about 2 seconds on Windows, and the follow loop wakes each second to see whether the service is alive.

## Turned down

- **Telling only a chat's start.** Owner's answer, "Yes, tell me again": a new approval or sign-in in a chat that already has a question open would never show.
- **A `chat_opened` command** and a mark cleared by opening the chat.
- **A notification for each new thing.** A burst in one chat would be many.
- **For knowing what is new:** the approval's `request_id` (only the chat's whole history carries it), a time of the newest need in `GET /api/chats` (a sign-in from a running turn keeps its turn's old time, so it would need a migration), and keying an approval by its turn (a second approval right after the first is answered would look the same).
- **The app following the live socket** instead of reading every 5 seconds.
- **A new route giving needs with reasons.**
- **The chat's id in the notification's `extra`.** Whether it survives the round trip on each platform wasn't confirmed; the notification's number was.
- **Sound, action buttons, a tray badge, withdrawing a sent notification, and Linux.**
- **Notifications from the dev window.**
- **A Gathr setting to turn them off.** The system's own notification settings do that.
- **Checking the Mac before merge** from the PR's release artifact on a test folder: a test folder changes the name a notification shows, and the download and a test account cost more than a missing notification found after it ships.

## Not checked

- That a real window reports its focus for a short visit and for a notification's click, on Windows and on macOS. Read in tauri 2.12.5's source only: on Windows the focus arrives as WebView2's focus, which wry gives the webview when the window gets the system's focus, whatever gave it; on macOS from the window becoming key. A programmatic `set_focus` goes the same way.

- The toast's name and icon, that it is silent, a click reaching the handler, and anything on macOS, including whether it asks for permission first.
- How a notification shows on a test data folder, whose identifier carries a `.data-<hash>` key.
- That the plugin's script, which it adds to every page, does nothing to Gathr's page. No capability grants the page its commands.
- What the reads cost on a real database every 5 seconds.
