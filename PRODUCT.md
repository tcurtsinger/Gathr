# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

Gathr is a Tauri 2 desktop app for Windows and macOS (WebView2 / WKWebView) that also runs in a local browser. Its UI is React in a webview and does not follow native Windows or macOS design language, so its design platform is `web`.

## Users

A mix of two groups, on both Mac and Windows:

- General-purpose AI users who use Claude Desktop (or similar) for everyday asks.
- Developers who use Claude Code and Codex.

Both want what those tools give them, plus the ability to use several accounts, run several agents at a time, and have agents review each other's work.

## Product Purpose

Gathr is an open-source desktop app for working with your own AI agents, from different providers (Claude Code, Codex and more), on your own computer and your own logins: a chat where you and your agents work like a team. It gives one place for an assistant for everyday asks, your own specialists, and a team with a lead when the work is a project, with tasks, plans, reviews and logging built in, so you can see how the agents work and make it better.

Success: people who would otherwise juggle Claude Desktop, Claude Code and Codex windows do that work in Gathr instead, with more accounts, more agents and real review than those tools give on their own.

## Positioning

- **Several accounts and providers in one place.** Each account has its own login folder; agents pick an account, model and effort. Claude Code and Codex agents work in the same chat.
- **Several agents at a time, as a team.** A chat holds one to five agents; team chats have a lead, and the team checks the lead's plan or answer in the open.
- **Reviewers built in.** Agents check each other in plain words rather than one model answering alone.
- **Local and visible.** Runs the providers' own agents on the person's own logins and computer; turns, tool calls and timings are recorded and can be inspected.

## Operating Context

- Desktop window (closes to tray / menu bar) or a local browser tab, on Windows and macOS.
- The window: a rail (Home, Agents, Memory, Plugins, Designs, Scheduled, Settings), the chat list, one centred chat (the room) and a resizable right panel of tabs (Home with Files, Designs, Changes and the board; a tab per opened file or design).
- A chat is a room: messages, replies folded under what they answer, reactions, a board of the work, and questions to the person. The assistant's chat never ends; projects get team chats.
- Agents schedule work, add tools and skills, keep notes in Memory, show tables, charts, diagrams and pages in the chat, and read or update pull requests.
- Outside AI apps connect through `gathr connect` and their messages show under their own name.
- System notifications tell the person when a chat needs them and the window isn't in front.

## Capabilities and Constraints

- Built today: see `docs/architecture.md`. Accepted but not built: remote use across computers, an agent browser, plan mode, catch-up digests.
- Nothing that costs money is required beyond the person's own provider accounts.
- Gathr starts no turn and writes no message on its own, except the team's check, scheduled work and a carry-on after Stop or an update.
- The way agents work as a team was restarted on 2026-11-20; `docs/archive/` is history, not rules.
- Words in the window are short and plain (DESIGN.md: Copy). No hover help text. Every setting has a control in Settings. No feature is designed around one particular agent.
- Product rules for building it live in `AGENTS.md`: the owner picks between two or three designs before anything they see is built.

## Brand Commitments

- Name: Gathr, always capitalised.
- Feel: a modern desktop AI interface.
- Reference for how the chat should feel: SerenityOS, the owner's earlier app, where agents talk, disagree and check each other in plain words.
- The assistant-ui React prototype is the visual and interaction reference (DESIGN.md, `designs/`).

## Evidence on Hand

- The product itself, open source on GitHub, with a release on every merge to `main`.
- No testimonials, user counts, benchmarks, press or case studies exist. Do not invent them.
- Screenshots and fixture data must be invented; never real conversations or personal data.

## Product Principles

1. **Many agents, one place.** Multiple accounts, providers and agents at once are the point; design for several voices in a chat, not one.
2. **Agents check each other.** Review and disagreement in the open are a feature to show, not noise to hide.
3. **Familiar to Claude Desktop and Claude Code users.** Someone coming from those tools should feel at home, then find more.
4. **Take out before adding.** Fewer screens, controls and messages; every addition is asked for.
5. **The person's own computer and accounts.** Nothing hidden, nothing that costs extra, nothing done on its own.

## Accessibility & Inclusion

Keyboard-first use and accessibility are explicitly not a focus for this product (owner's decision, 2026-11-23). Do not let them drive design choices; meet the baseline the components already provide.
