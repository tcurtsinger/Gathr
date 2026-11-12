# 0041. A step shows what it ran

Status: accepted (owner, 2026-11-11, on the plan for the chat redesign's fifth slice). Date: 2026-11-11.
It ends 0037's line "Steps show no argument … until the service keeps them". Thinking is a later slice.

## Choice

- **A tool message keeps what it ran, inside its body:** `{"tool", "status", "duration_ms", "input"}`,
  `input` being `{"text", "title"}`. No new column and no migration: the body is JSON already, the API and
  the live socket send it whole, and search never indexes it.
- **`text` is one line, at most 200 characters** (the owner's call): a command's first line, a file's
  path, a search's words. `title` is the engine's own words for the call when it gives some, at most 80.
  Where more followed, the text ends with "…". A path is kept as the engine gave it.
- **Secrets are hidden with the rule Diagnostics uses** (the owner's call), `gathr::redact`, before the
  text is cut, so a key the cut would split is still seen whole. One function, `gathr::steps::input`,
  does the line, the hiding and the cut for both engines.
- **Each driver picks the field, and only for a tool whose input was seen in real frames:**

  | Tool | `text` | `title` | Seen in |
  |---|---|---|---|
  | Claude `Bash`, `PowerShell` | `command` | `description`, when given | `background.jsonl`; live: 1,798 and 30 |
  | Claude `Read`, `Write`, `Edit` | `file_path` | | `approval.jsonl`, `subagent.jsonl`; live: 109, 47, 61 |
  | Claude `Grep` | `pattern` | | live: 22, all a sub-agent's |
  | Claude `WebFetch` | `url` | | live: 5, all a sub-agent's |
  | Claude `Agent` | `description` | | `subagent.jsonl`; live: 5 |
  | Codex `commandExecution` | `command`, out of its shell | | live: 541 |
  | Codex `fileChange` | the first `changes[].path`, then "+N more" | | live: 35 |
  | Codex `webSearch` | `query` | | live: 1 |

  "Live" is the owner's own Gathr database, read on 2026-11-11 with the owner's leave, read-only, for key
  names, value types and counts only: the number is how many tool calls of that kind were looked at.
  Gathr's own tools, every other MCP tool and any tool not listed keep nothing: MCP arguments are
  arbitrary, and a task Gathr gave already shows from the task itself.
- **Write's `content`, Edit's strings, a sub-agent's prompt and a web page's prompt are never kept.**
- **Codex's command comes out of the shell it is wrapped in.** Codex hands over the shell's command line
  with its words quoted as a POSIX shell quotes them, on Windows too
  (`"C:\\…\\pwsh.exe" -Command 'git status'`). The driver splits it as a POSIX shell would and keeps the
  script when the line is a shell (`pwsh`, `powershell`, `bash`, `zsh`, `sh`), then only flags, then one
  script. Any other line stays as it is.
- **Codex's web search says its words only as it ends:** `query` is empty at `item/started`. So a tool's
  end may carry what it ran, and the end's body wins over the start's.
- **A sub-agent's own tools stay under its Agent call** (G24), with their input, never as the agent's steps.
- **A Codex item that ends with no `status` did not fail.** A web search and an image view end with
  neither `status` nor `success` (34 live items); the driver read that as a failure, and the step would
  have shown "Failed". Fixed here, with its test.
- **The window** shows the text as a chip after the step's label, in monospace, cut to the column with an
  ellipsis and whole in the shared tooltip when pointed at. The label is the engine's title when there is
  one, else a short one beside the chip ("Ran", "Read", "Edited", "Searched", "Fetched", "Searched the
  web", "Ran a sub-agent"). A step with no input reads as before ("Ran a command"), and so does the row's
  own label while a turn runs.
- **Old messages have no input** and show their label alone. No backfill.
- **Found from Diagnostics** (0019). Each `execute_tool <name>` event carries the text as
  `gathr.tool.input`, so `gathr diag trace <ref>` lists what ran. A listed tool whose frame lacks its
  field keeps nothing and records `gathr.engine.tool_input` with error `shape_changed`, the tool's name
  and the field, on the turn's reference: a protocol change is found there, not by a person noticing a
  chip gone.

## Two gaps in `redact`, fixed here

- **`--token=abc` was not hidden** (`--token abc` was). A flag whose name holds `token`, `key`, `secret`
  or `password` now hides its value after `=` too: `--token=[redacted:flag]`. The flag's name decides, as
  it does for the form with a space, so `--port=47101` and any other `--flag=value` stay as written.
- **Redacting twice changed the text.** A step's text is redacted when kept and again on the event that
  carries it; the second pass took the word "bearer" inside `[redacted:bearer]` for the word before a
  secret and hid the next word. A `[redacted:<kind>]` mark is now passed over whole, and stands for the
  secret a `--token` or `Bearer` before it was waiting for. This affected `engine_raw` text copied into
  events too.

## Why

- A row that says "Ran a command" six times says nothing about what was done (S25, G142). The command's
  first line is the name a person would give the step (G150).
- The chat is screenshotted, searched by agents and shared more than Diagnostics is, so it gets the same
  hiding. The cost the owner accepted: an ordinary `NAME=value` before a command shows as
  `NAME=[redacted:env]`.
- The drivers know their engines; one shared function means one cut and one rule.
- The input is in the frame the driver already reads: no extra frames, nothing to wait for.

## Turned down

- **The whole command, up to 2,000 characters, opened by a click.** Ten times the size per step, and whole
  scripts, which hold more secrets, kept in the chat.
- **Only the program's name.** "git merge origin/main" becomes "git".
- **A new column or table.** It buys nothing over the body.
- **Showing it unredacted,** as the approval card shows a raw command.
- **Cutting before hiding,** as the plan first had it: a key that straddles the 200th character would
  have lost the length that marks it as a key, and kept its start.
- **Trusting Codex's own `commandActions[].command`** for the script: it held the script for 403 of the
  541 commands and the whole wrapped line for the rest.
- **A backfill from `engine_raw`.** Code for one day's use, and the raw frames are to be pruned.

## Tests and their frames

`steps_claude.jsonl`, `steps_codex.jsonl` and `steps_shape_changed.jsonl` in `crates/gathr/tests/frames`
are **hand-written, not recordings**: made-up commands, paths, URLs and queries in the shape verified
above, each saying so in its first row. Nothing in them comes from a real conversation. The real
recordings (`background`, `approval`, `subagent`) cover the tools they hold.

## Cost

Measured on the fixtures through the service. A body without input is 45 to 66 bytes; one with input was
75 to 168. A Claude turn of 11 steps went from 564 bytes of tool bodies to 1,001; a Codex turn of 7 from
349 to 633. The most a step can add is about 300 characters. No extra frames and nothing to wait for.

## Not built, not verified

- **Claude's `NotebookEdit`, `Glob` and `WebSearch`:** no real frame of them was seen, so they keep
  nothing. Each is one line in the driver once a frame shows its field.
- **Tools seen live that the plan did not list keep nothing** until the owner decides: Claude's `Monitor`
  (7), `ToolSearch` (24), `TaskStop` (10), `SubagentHandback` (5); Codex's `imageView` (33).
- **Codex's shell on macOS** (`zsh -lc`, as SerenityOS's code has it) was not seen: every live command was
  `pwsh.exe -Command` on Windows. The same split handles it, by the rule above, but no real frame proves it.
- **Codex's web search** rests on one real call.
- **How many steps a real turn has,** so the cost per turn.
