# 0005. One MCP tool server inside the service, over streamable HTTP, with rmcp

Status: accepted. Date: 2026-11-06.

Built: yes. Proof: `crates/gathr/tests/tools.rs`, `crates/gathr/tests/coordination.rs`, `crates/gathr/tests/diag_tool.rs`.

Out of date below: the status below leaves out tools built since: GitHub, merge, checks and settings ([0043](0043-checks-and-merge.md), [0046](0046-github-tools.md)).

## Implementation status

The in-process server, per-engine tokens, chat reading, coordination and diagnostics tools are built. The `diag` tool answers `health`, `failures` and `trace` for the caller's chat (0019).

## Choice

- The service serves Gathr's tools to every agent from one MCP server at `http://127.0.0.1:<port>/mcp`, on the same axum server as the API (0004).
- Crate: [`rmcp`](https://crates.io/crates/rmcp), the official Rust MCP SDK, with `server`, `macros` and `transport-streamable-http-server`.
- **Each agent gets its own token,** made when its engine starts and dropped when it exits. The token tells the server which agent is calling, so a tool like "give a task" or "wait" acts as that agent, and every service tool call is a `gathr.tool.call` event in the record (0011) under that agent and turn.
- **Claude Code** gets it through `--mcp-config` with `{"type":"http","url":...,"headers":{"Authorization":"Bearer ${GATHR_AGENT_TOKEN}"}}` and `--strict-mcp-config`, the token in the engine's environment.
- **Codex** gets it through `-c` overrides for `mcp_servers.gathr.url` and `mcp_servers.gathr.bearer_token_env_var = "GATHR_AGENT_TOKEN"`, so the token is never written into its `config.toml`.
- Tool approvals are not an MCP tool: they come over each engine's own protocol (0001).

## Why

- **One server, no extra processes.** Streamable HTTP is MCP's transport for servers that serve many clients ([transports, 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)). rmcp's `StreamableHttpService` is a Tower service that mounts on an axum router ([rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)). The previous app ran an MCP server per agent and bridged it back over a socket to approve tools; one in-process server removes the bridge.
- **Both engines speak it with a bearer header.** Claude Code: HTTP servers in `--mcp-config` with `headers`, and `${VAR}` expanded from the environment ([Claude Code MCP](https://code.claude.com/docs/en/mcp)). Codex: `[mcp_servers.<name>] url` with `bearer_token_env_var` ([Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)).
- **rmcp is current and official.** 3.9.0, released 2026-11-05, maintained under the modelcontextprotocol organisation; its README says it implements the 2026-07-28 spec and stays compatible with earlier revisions ([crates.io](https://crates.io/crates/rmcp)).
- **Licence:** rmcp is Apache-2.0 only, which an MIT OR Apache-2.0 project can use; release builds carry its notice.
- **Per-agent tokens** turn "who called this tool" from a guess into state, which coordination as state needs (0010), and let a token be revoked when its engine stops.

## Rules that follow

- The MCP endpoint applies the same `Host` and `Origin` checks as the rest of the API (0004); the MCP spec requires the `Origin` check ([transports](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)).
- The server accepts both the 2026-07-28 revision and the 2025 revisions with sessions. Claude Code uses 2026-07-28 where the server supports it; Codex pins rmcp 3.5.1 ([codex-rs/Cargo.toml](https://github.com/openai/codex/blob/main/codex-rs/Cargo.toml)), and which revision it speaks is checked in the first slice.
- Each tool has a test that calls it over HTTP the way an engine does.
- Tool names and descriptions are generic (AGENTS.md: agents are generic): no tool is written for one engine.

## Turned down

- **A stdio MCP server per agent** (the previous app's approach): a process per agent and a bridge back to the service.
- **The old HTTP+SSE transport:** deprecated in the spec and in Claude Code; rmcp doesn't offer it ([rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)).
- **Writing the protocol ourselves.** MCP moves fast (2025-11-25 to 2026-07-28 dropped sessions and the initialize handshake ([versioning](https://modelcontextprotocol.io/specification/versioning))); the official SDK tracks it.
- **One shared token for all agents.** The server couldn't tell agents apart.

## Checked

- Claude Code 2.1.318 on Windows, signed in (slice 2, F3): with `--mcp-config` as above, `system/init` showed `gathr` as `connected`, so `${GATHR_AGENT_TOKEN}` in the header is filled in from the environment; the guard refuses any other token. Claude Code defers MCP tools behind its `ToolSearch`, so an agent finds a Gathr tool only by searching for it: each engine's briefing names Gathr's tools (0021).

## Not verified

- The exact revision negotiated by Codex. Its MCP task/finish/hear-back flow worked on a signed-in team after 0033 landed; that did not record the negotiated revision.
- Which revision Claude Code 2.1.318 speaks to it: the server answered it, but its requests weren't recorded.
