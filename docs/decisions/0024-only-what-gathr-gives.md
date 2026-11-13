# 0024. Agents get only what Gathr gives them

Status: accepted (the owner, 2026-11-07). Date: 2026-11-07.

Built: yes. Proof: `crates/gathr/src/claude/frames.rs`, `crates/gathr/tests/tools.rs`. Left for later by the decision itself: bringing in the person's own items, per agent.

Out of date below: Codex is built and its switches are set ([0033](0033-codex-driver.md)), and a Gathr folder has been signed in to Codex and run; five account tools are denied, the ones named.

## Choice

- An agent's engine loads nothing from the person's own setup or accounts: no plugins (synced from claude.ai or installed), no hooks, no skills or commands, no MCP servers or claude.ai connectors, no sub-agents of their own, no auto-memory. What an agent has comes from Gathr.
- Something the person wants from Claude or ChatGPT is imported into Gathr, where it's seen and switched on per agent in Settings. Off by default. The import itself is a later feature; until then agents run without.
- The project's own files still apply: `CLAUDE.md`, `AGENTS.md` (0021) and the project's `.claude` settings. They belong to the project, not to the person's account.
- The engine's own built-in tools stay (0001). Tools that come with a signed-in account and act outside Gathr are denied, each by name, in the driver's deny list.

How, for Claude Code (2.1.318), ported from the previous app's `CLEAN` (`desktop/server/drivers/claude.ts`, the owner's code):

| What | Switch |
|---|---|
| Skills and commands | `--disable-slash-commands` |
| MCP servers, plugins' servers | `--strict-mcp-config`, with only Gathr's servers in `--mcp-config` |
| claude.ai connectors (18 on the owner's account) | `disableClaudeAiConnectors: true` in `--settings`: documented for connectors; `--strict-mcp-config` kept them out live too, but its docs don't say it covers them |
| Plugins (synced and installed), their hooks, the person's sub-agents and settings | `--setting-sources project,local` |
| Auto-memory | `autoMemoryEnabled: false` in `--settings` |
| Account tools: `PublishPlugin`, `PushNotification`, `SendToProject`, `SendToProjectContext`, `ShareOnboardingGuide` | `--disallowedTools` |

For Codex, when its driver lands, the previous app's `codexClean` (`desktop/server/drivers/codex.ts`) is the starting point: `features.plugins=false`, `features.apps=false`, `features.hooks=false`, `skills.include_instructions=false`, `skills.bundled.enabled=false`, `notify=[]`, and every server of the person's `config.toml` switched off except Gathr's. It's proved live the same way before it's relied on.

**Oldest Claude Code:** 2.1.318, the version all of this was proven on. Gathr reads `claude --version` each time it starts an engine and fails an older one by its version, with a reference ("Claude Code 2.1.183 is too old: Gathr needs 2.1.318 or later"). Claude Code's changelog shows why older ones can't simply be allowed: `--forward-subagent-text` came in 2.1.211, `AGENTS.md` in 2.1.277, and `--setting-sources` stopped letting an excluded file supply some settings in 2.1.314. Turned down: 2.1.314, the oldest the changelog allows, because the `cc-plugin-agents-md` option that loads both instruction files isn't in the changelog, so nothing older than 2.1.318 is known to load them.

## Why

- **The owner's call (task 856):** "The only things you get from Gathr should be what's in Gathr. Nothing from outside."
- **Measured, signed in to the owner's account:** without these switches a session loaded 15 synced plugins, 75 skills, 114 commands and 6 account tools, and a plugin's `SessionStart` hook added 13,676 characters to every session. That context nobody chose in Gathr is what 0020 rules out, and a hook's text lands as if it were an instruction.
- **Already solved:** the previous app starts Claude Code with these switches, for the same reason ("A plugin's session-start text would otherwise land in the context as if it were an order").

## Checked

Live, Claude Code 2.1.318 on Windows, in Gathr's signed-in test folder:
- Without the switches (the part B driver): 15 synced plugins, 75 skills, 114 commands, 2 `SessionStart` hook runs, and the 6 account tools. `claude mcp list` in the folder shows 18 claude.ai connectors and 5 plugin servers.
- With all the switches above, `system/init` lists only Claude Code's built-in `cc-plugin-*` plugins, no skills, no commands, no MCP servers, and no hook runs. Recorded through the service as `crates/gathr/tests/frames/signed_in.jsonl`; a test fails if any of it comes back, and it fails on a recording made without the switches.
- `CLAUDE.md` and `AGENTS.md` both still load with the same switches: each held a made-up marker word, and the model named both.
- Codex: whether its login brings anything in the same way isn't checked yet; no Gathr folder is signed in to Codex.
- `--safe-mode` was tried first and turned down: it also drops `CLAUDE.md` and `AGENTS.md`.

## Turned down

- **`--safe-mode`:** keeps nothing from the project's instruction files.
- **`--bare`:** never reads a subscription login.
- **Setting each switch in the account folder's `settings.json`:** the folder is the person's login; Gathr writes nothing there (0021).
- **Per-item switches** (`syncClaudeAiPlugins`, `disableAllHooks`, `disableClaudeAiConnectors`): each covers one kind; `--setting-sources` covers them together, and the recording proves it.

## Not verified

- macOS: the same switches with a Keychain login.
- Managed (organisation) settings still apply under every switch above, by Claude Code's design; nothing here was run on a managed machine.
- Codex: nothing run yet.
