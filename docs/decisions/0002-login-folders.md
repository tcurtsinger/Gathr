# 0002. One login folder per agent, owned by one process, never copied, never killed mid-renewal

Status: proposed. Date: 2026-11-06.

## Choice

- Each agent gets its own engine folder under Gathr's data folder: `<data>/agents/<agent-id>/claude` passed as `CLAUDE_CONFIG_DIR`, or `<data>/agents/<agent-id>/codex` passed as `CODEX_HOME`. The person signs that agent in once, through the engine's own login flow, inside that folder.
- At most one engine process runs on a folder at a time. The service enforces it: the folder is the lock, and a second start for the same agent waits or reuses the running process.
- Gathr never reads a login's secret, never copies a login file, and never touches `~/.claude` or `~/.codex`.
- Settings may be copied in when an agent is created (Claude: `settings.json`, `CLAUDE.md`, skills, agents, commands, output styles; Codex: `config.toml`), each at the person's choice. Login files are never copied: Claude's `.credentials.json` and `.claude.json`, Codex's `auth.json` and `.sandbox-secrets`.
- Login state is read without starting a turn: for Claude, `claude auth status` (exits 0 signed in, 1 signed out, prints JSON) on macOS, and the presence and expiry fields of the login file elsewhere, never its tokens; for Codex, `account/read` on the agent's own app-server.
- The engine's environment is cleaned before spawn: `ANTHROPIC_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN`, and `OPENAI_API_KEY` are removed unless the person chose a key for that agent.

## Why

- **Refresh tokens are single-use.** Codex refreshes a token 5 minutes before it expires and a reused refresh token fails with `refresh_token_reused`, forcing a new sign-in (`codex-rs/login/src/auth/manager.rs` in [openai/codex](https://github.com/openai/codex); [Codex auth docs](https://learn.chatgpt.com/docs/auth): tokens live in `auth.json` under `CODEX_HOME` and refresh automatically). The previous app hit the same with Claude Code: two processes renewing at once, or one killed between renewing and saving, leave a spent token, and the next renewal blanks the login. One process per folder removes the race instead of managing it.
- **The folder variables move the login.** `CLAUDE_CONFIG_DIR` moves settings, history and `.credentials.json` ([env vars](https://code.claude.com/docs/en/env-vars)); `CODEX_HOME` holds `auth.json` and `config.toml` ([Codex auth](https://learn.chatgpt.com/docs/auth)), and app-server reports the folder it uses as `codexHome` in its `initialize` response (generated schema, codex-cli 0.166.0).
- **Codex can sign in from inside Gathr.** app-server has `account/login/start` with `chatgpt` (returns an `authUrl`), `chatgptDeviceCode` and `apiKey`, and announces `account/login/completed` ([app-server docs](https://learn.chatgpt.com/docs/app-server)). Claude Code is signed in by running its own `claude /login` in that folder.
- **An API key in the environment wins.** In `-p` mode `ANTHROPIC_API_KEY` takes precedence over a subscription login ([authentication](https://code.claude.com/docs/en/authentication)), so a key left in the person's shell would silently switch billing.
- **Ported from the previous app (the owner's code):** the folder layout, settings copied in and logins never (`drivers/claude-folder.ts`, `drivers/codex-folder.ts`), reading login state from the file instead of starting Claude Code (`drivers/claude-login.ts`), and watching the folder, not the file, since Claude Code replaces the file when it saves, with a once-a-minute re-read because Windows can drop a change event (`login-watch.ts`). That watcher exists because a cleared login was once noticed 29 hours late.

## Rules that follow

- **Never force-kill an engine that may be renewing.** Stop and close go through the engine's interrupt (0001). A kill after the grace period is written to the event record with the agent, the process and why, so a lost login can be traced to it. This replaces the previous app's `engine-trail.ts`, which kept 10 minutes of starts and kills in memory for the same purpose.
- **Sign-out is an event.** When the watcher sees a login disappear or expire, the agent shows as signed out, its queued work waits, and the event record says when and what ran just before.
- **Status checks are never killed.** `claude auth status` may itself renew; the service waits for it to exit on its own (the previous app waited up to 30 s, then trusted the file).
- **Several agents can share one account,** each with its own sign-in in its own folder. Each sign-in is its own grant, so none can spend another's token.
- **Folders are deleted only with the agent,** and only after its process has exited.

## Turned down

- **One folder per account, shared by all of that account's agents, with a renewal gate** (the previous app: one renewer at a time, others wait up to 30 s). It saves signing in per agent, but the gate covers only process starts; a long-lived process renews mid-turn on its own, and the race remains. AGENTS.md: "Never share one between processes".
- **The person's own `~/.claude` or `~/.codex`.** Their terminal sessions would race Gathr's engines for the same token.
- **A long-lived token** (`claude setup-token`, one year, via `CLAUDE_CODE_OAUTH_TOKEN` ([authentication](https://code.claude.com/docs/en/authentication))). It avoids renewal entirely but is a copyable secret Gathr would have to store; kept as an opt-in for a later decision, not the default.
- **Gathr running the OAuth flow itself.** That would mean Gathr holding tokens; the engines' own flows keep the secret inside the engine's folder.

## Open, for the owner

Anthropic's Agent SDK page says: "Unless previously approved, Anthropic does not allow third party developers to offer claude.ai login or rate limits for their products, including agents built on the Claude Agent SDK" ([overview](https://code.claude.com/docs/en/agent-sdk/overview)). Gathr doesn't offer a login: the person signs in to their own Claude Code, which Gathr runs on their machine. Whether that counts is the owner's call, as a decision task.

## Not verified

- Whether `CLAUDE_CONFIG_DIR` also moves `~/.claude.json`, and where keyless Console sign-ins are stored (the docs say outside the folder). The first slice checks both with a test login folder before any real sign-in.
- Claude Code's own refresh timing isn't documented; the 5-minute figure is Codex's, and the previous app's observation for Claude.
