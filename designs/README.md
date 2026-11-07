# Gathr design candidates

Phase 0 previews, not a product implementation. All activity and timings are fictional. No assets or styles are taken from the previous app.

Each screen has three matching versions:

| Version | Direction | What leads | Type | Accent (light / dark) | Dials (variance, motion, density) |
| --- | --- | --- | --- | --- | --- |
| v1 | Ledger | Queue, event stream, five-agent status strip | Calibri, system fallback | #744e17 / #e3c18d | 2, 1, 7 |
| v2 | Workbench | Project rail, conversation, current work beside it | Segoe UI, system fallback | #245d78 / #a1cde1 | 2, 1, 6 |
| v3 | Dispatch | Work grouped by state, conversation below | Verdana, system fallback | #365c38 / #b2d2ad | 3, 1, 5 |

Subject: desktop coordination of agents from multiple providers. Audience: a person supervising concurrent project work in daylight or evening light. Primary job: see who is working, resolve a blocker, and continue a conversation. Mode: Operate. All directions use restrained neutrals and one action accent. Body 16px, task/sidebar text 14px, metadata 12px, titles 28px (Dispatch 32px), 4px spacing base. Light and dark are paired, not inverted.

Open main-window/v1.html, room/v1.html or chat/v1.html in the design viewer. Match version numbers across screens. Use Dark to compare the paired theme. Navigation links move among the three representative screens. Links for other sample projects and conversations reuse these representative screens.

Local preview interactions: Pause/Resume, Stop/Resume, disclosures for tasks, side jobs, history, tools and diagnostics; queue removal; composer feedback; theme selection. Create and Review show preview feedback. They do not persist or contact agents. No live loading or failure state is simulated here. The selected system must specify those before production.

Room fixtures include 216 earlier messages, five active-room members across two providers, a side job inside its asker's work, a named wait with its start time, a paused member and a plan check. Overview includes twelve agents (seven behind a disclosure in Workbench and Dispatch).

Task anatomy is title, owner, state, with file paths and acceptance criteria behind disclosure. All three directions use the same task rows in room and chat. Dispatch groups those rows into work cards by state.

Check from the repository root:

    node designs/check.cjs

This checks scripts, IDs, local-only content, history size, and declared theme-role contrast. Use the app's design check and screenshot commands for every page as well. That check cannot validate canonical colors until a direction is picked. DESIGN.md and designs/system.html are intentionally deferred to the pick, per the approved plan.

No before screen exists. No production latency is claimed. No packages, fonts or other assets need downloading.

## Round 2 candidates

See ROUND2.md for v4 Signal, v5 Folio and v6 Relay. These revise visual language, conversation density, state treatments and agent identities. Run node designs/check-round2.cjs as well as the original check.
