# 0044. A step keeps what it returned, and a command whole

Status: accepted (owner, 2026-11-12, room tasks 140-146, on `Shared\plans\step-results.md`). Date: 2026-11-12.
0043 is the checks-and-merge decision, so this one is 0044. It extends 0041 (what a step ran) for the work
row's second slice: assistant-ui's Tool call shows a Request and a Result.

## Choice

- **A step's body gains `result`**, set as the step ends: `{"text", "lines", "more"}`. `text` is at most
  5 lines (the owner's call), each hidden with `gathr::redact` and then cut to 200 characters, as 0041
  cuts its line. `lines` is how many lines came back, and `more` whether `text` left some out. A failed
  step keeps its error the same way. A step with nothing to say keeps no `result`.
- **What each kind of step keeps** (the owner's picks, from the plan's R3 to R8):
  - **A command** (Claude Code's `Bash`/`PowerShell`, Codex's commands): its output's last lines, where
    a build or a test run ends with its summary or its error. When it failed, Codex's exit code follows
    as `exit N`, which is one of the five lines. Claude Code gives no exit code: none was seen live,
    so none is shown.
  - **A read:** how many lines were read ("4 lines"), never what they hold. A file's body is where
    secrets sit that `redact` doesn't know. When Claude Code doesn't say what kind of step it was (a
    frame that ends several calls has no `tool_use_result` to say whose it is), a step that reads or
    writes a file (`Read`, `Write`, `Edit`, `MultiEdit`, `NotebookEdit`) keeps no result at all: its
    text could be the file's own lines (#161's review). Other tools keep their first lines.
  - **An edit or a new file:** lines added and removed ("+1 −1"; for Codex, one line per file with its
    path).
  - **A search** (`Grep`, `Glob`; Codex's web search): how many it found, then its first hits ("1 file",
    then `notes.txt`). A web search says only how many results.
  - **Anything else** (Gathr's own tools, a sub-agent's report, other MCP tools): the first lines of its
    answer.
- **A command keeps its whole text as the Request** (`input.request`, the owner's call): up to 8 lines
  and 2,000 characters, hidden before the cut. It's kept only when it says more than 0041's one line (a
  script, or a line cut at 200), so a one-line command isn't stored twice. The chip stays 0041's line.
- **No time per step** (the owner's call). `duration_ms` stays in the body, as it was. The window
  shows the turn's total.
- **Read from the frames the drivers already read.** For Claude Code, the `tool_result` block (what the
  model was given) and, beside it, Claude Code's own `tool_use_result`, which says what kind of step it
  was: `stdout`/`stderr`, `file.numLines`, `structuredPatch`, `numFiles`/`filenames`/`numLines`. It's
  read only when the frame ends a single call, since with several it can't say whose it is, and then the
  text answers alone. For Codex, `item/completed`: `aggregatedOutput` and `exitCode`, `changes[].diff`,
  `results`, and `result.content` or `error`.
- **As 0041:** in the message body only. No migration, nothing in the event record (0011), so nothing
  reaches a support bundle. No backfill. A sub-agent's steps keep nothing.

## Measured first (read-only, lengths and counts only, the owner's leave, room task 146)

The owner's Gathr, the agents' own calls (`Shared\evidence\step-results\`):

- Claude Code: `Bash` 1,803 results, on average 22 lines and 1,218 characters, 322 over 2,000; `Read` 83,
  on average 17 lines; `read_chat` 20, on average 12 KB; `Edit`, `Write` and most of Gathr's tools 1 or
  2 lines.
- Codex: commands 583, on average 99 lines and 5,767 characters, 80 with a non-zero exit; Gathr's tools
  309, on average 5 lines; file changes 46 (73 files: `path`, `kind`, `diff`).

So 5 lines cut mostly command output and `read_chat`. Everything else fits whole.

## Recorded

`tests/frames/results_claude.jsonl` and `results_codex.jsonl` are **real turns** on test accounts
(Claude Code 2.1.322, Codex 0.167.2), scrubbed with `scripts/scrub-frames.mjs` and read before commit.
Codex's turn looked for `AGENTS.md` up through real folders on the PC, which the scrub doesn't reach,
so those paths were renamed by hand (`C:\work\work\agent\…`). Claude's turn holds `PowerShell`, `Write`,
`Read`, `Edit`, `Grep`, `Glob` and Gathr's `list_tasks`. Codex's holds commands (one failing) and
Gathr's tools. The test accounts were signed out and their folders deleted after.

## Turned down

- **Keeping the result whole**, or up to 2,000 characters a line: the measured command output averages 5.8 KB
  on Codex, and whole outputs hold more of what `redact` doesn't know.
- **A file read's first lines:** a `.env` or a config is where an unknown secret sits.
- **A diff's lines for an edit:** the same reason, and the `+N −M` reads at a glance.
- **Claude Code's exit code from the text** ("Exit code 1" in a failed command's answer): not seen live,
  so not built on.
- **The whole command for every command:** a one-line command's Request is its line already.

## Not verified

- A failed Claude Code command's frames (its `tool_use_result` and text): none in the recording.
- Codex's file change and web search results live: only their field names and counts were seen
  (`changes[].diff`, `results`). They're read as those names say, and say less if their shape differs.
- Claude Code's `WebSearch`, `WebFetch` and `NotebookEdit` results, and a sub-agent's report through
  `tool_use_result`: their text answers as anything else does.
