# 0041. A step shows what it ran

Status: accepted (owner, 2026-11-11, on the plan for the chat redesign's fifth slice). Date: 2026-11-11.
It ends 0037's line "Steps show no argument … until the service keeps them". Thinking is a later slice.

Built: yes. Proof: `crates/gathr/tests/steps.rs`, `ui/src/chat/messages.test.ts`.

Out of date below: a step opens to its Request instead of showing a tooltip, and keeps its request and result ([0044](0044-what-a-step-returned.md)); thinking is built ([0042](0042-thinking.md)).

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
  script when the line is a shell (`pwsh`, `powershell`, `bash`, `zsh`, `sh`), flags that end in a command
  flag (`-Command`, `-c`, or short flags run together ending in `c`, as `-lc`), then one script. Any other
  line stays as it is: `bash -l build.sh` and `pwsh -File build.ps1` run a file, and keep their shell.
- **Codex's web search says its words only as it ends:** `query` is empty at `item/started`. So a tool's
  end may carry what it ran, and the end's body wins over the start's.
- **A sub-agent's steps are not kept or shown yet.** Its tools stay under its Agent call, never as the
  agent's own (G24), and the service drops them: the chat shows the Agent step alone. Their input is not
  worked out. A field one of them lacks is still recorded as below, on the agent's turn. The live Grep and
  WebFetch calls were all a sub-agent's: they verify the shape the agent's own calls keep.
- **A Codex item that ends with no `status` did not fail.** A web search and an image view end with
  neither `status` nor `success` (34 live items); the driver read that as a failure, and the step would
  have shown "Failed". Fixed here, with its test.
- **The window** shows the text as a chip after the step's label, in monospace, cut to the column with an
  ellipsis and whole in the shared tooltip when pointed at or focused: a cut chip is a tab stop, a whole
  one is not. The label is the engine's title when there is one, else a short one beside the chip ("Ran",
  "Read", "Edited"). A step with no input reads as before ("Ran a command"), and so does the row's own
  label while a turn runs. Grep, WebFetch, WebSearch and Agent read "Searched", "Fetched", "Searched the
  web" and "Ran a sub-agent" with or without a chip (and "Searching the web" while a search runs, before
  Codex names it), where they showed the tool's name.
- **Old messages have no input** and show their label alone. No backfill.
- **The text lives in the message only, never in the event record** (0011: events never copy content,
  and a support bundle exports them). `execute_tool <name>` says the tool's name, as before, so nothing a
  step ran reaches a bundle. `gathr diag trace` is as it was: what ran is read in the chat.
- **Found from Diagnostics** (0019). A listed tool whose frame lacks its field keeps nothing and records
  `gathr.engine.tool_input` with error `shape_changed`, the tool's name and the field, on the turn's
  reference: a protocol change is found there, not by a person noticing a chip gone.
- **A cut never lands inside a `[redacted:…]` mark:** it backs up to before it. One helper,
  `gathr::text`, takes the first line and cuts it, for a step and for a chat's title.

## What `redact` hides, widened here

`redact` is one pass, shared with Diagnostics and the raw frames, so each of these helps there too. Each
has its test, with look-alikes proven untouched (`crates/gathr/src/redact/tests.rs`).

- **A secret flag's whole value,** after a space or an `=`: to the next space outside quotes, whatever
  punctuation it holds (`--auth-token=user:s3cret`, `--token 'a b'`). `--token=abc` was not hidden at all
  before. The value's own quotes stay around the mark.
- **Raw JSON text stays JSON** in the cases tested (streamed tool input joined, a frame that didn't
  parse; both go on to `diag turn --raw`): a wait for a secret ends at a `}` or `]` that closes, and at a quote
  after which the text goes on as JSON does (a closing bracket, the end, or `,` or `:` and a string, an
  object, an array or a whole literal). A bare value ends at `}` and `]`. So
  `{"pattern":"Bearer ","path":"src"}` and `{"command":"x --token"}` are left whole, and
  `{"args":["--token","abc"]}` hides `abc` inside its quotes. As before this change, a wait still crosses
  `", "` into the next string (`{"a": "Bearer", "b": "c"}` hides `b`): hidden too much, still JSON.
- **The flag's name is read word by word,** split at `-`, `_`, `.` and capitals: `token`, `key`, `secret`,
  `password`, `passwd`, `auth`, `credential(s)`, and `pass` whole. A word run together counts when it
  ends in `token`, `secret`, `password` or `passwd` (`--accesstoken`, `--clientsecret`), or is `key` after
  a word ending in `api`, `access`, `private`, `secret` or `auth` (`--apikey`, `--secretaccesskey`). So `--keyboard`, `--monkey`,
  `--hotkey`, `--tokenizer` and `--max-tokens` stay. `--key-file=path`, `--sort-key=name` and `sort --key=2,2` are
  hidden though they hold no secret: hiding too much is the safe side. `--pwd` is left: it is as often a
  folder. `--password-stdin` and `--with-token` take no value, so the word after them stays.
- **PowerShell parameters,** one dash: `-Token`, `-ApiKey`, `-AccessToken`, `-Password`, `-Secret`,
  `-Credential`, and `-Token:value`. Not `-Path`, `-Name`, `-KeyboardLayout`.
- **`$env:NAME = "value"`** when the name holds a secret word. An upper-case `NAME=value` was hidden
  already, whatever its name; now also when its value is quoted.
- **A password in a URL or connection string:** `scheme://user:[redacted:url]@host`. The name and host
  stay, also where the URL is glued to what comes before it (`--index-url=https://…`, `<https://…>`). A
  provider's key standing as the whole login goes too. A secret flag's or an environment value that is a
  URL is hidden whole, as before.
- **Headers in plain text:** `Authorization` with any scheme (`token`, `Basic`; `Bearer` already), a
  hyphened name with a secret word (`X-Api-Key`, `X-Auth-Token`) and `apikey`. A lone word before a colon
  (`key: value`, `error: token expired`) is prose, and stays. With no space after the colon, as curl
  takes it, only `Authorization:` and `X-…:` names are read (`re-auth:needed` is prose). In a frame's
  JSON, a key such as `X-Api-Key` is hidden whole like `api_key`.
- **Keys by how they start,** at 20 characters or more: `sk-` (already), `sk_live_`, `sk_test_`, `ghp_`,
  `gho_`, `ghu_`, `ghs_`, `github_pat_`, `xoxb-`, `xoxp-`; and `AKIA` with 16 capitals or digits. Also
  after an `=` (`?access=ghp_…`).
- **Redacting twice changes nothing.** The second pass took "bearer" inside `[redacted:bearer]` for the
  word before a secret. A mark is passed over whole, wherever it stands, and stands for the secret a
  `--token` or `Bearer` before it was waiting for.

Known limits, left as they are:

- `-p hunter2`, `mysql -pSECRET`, `curl -u user:pass`, a bare AWS secret access key, a private key's body
  (only the first line is kept anyway), `--pwd`.
- `name=value` with a lower-case secret name (`?token=abc`, `password=abc`), unless the value is a known
  key.
- A value is cut at a backslash and at quotes joined on by the shell: `--password=ab\cd` hides `ab`,
  `--token="ab"'cd'` hides `"ab"`.
- Escaped quotes inside raw JSON stream fragments: `--token \"a b\"` hides only `a` there. The finished
  frame is redacted value by value, and is whole.
- Run-together names that end in `key` after any other word (`--sshkey`, `--publickey`, `--hostkey`;
  a word ending in `api`, `access`, `private`, `secret` or `auth` before `key` counts), and
  plurals (`--tokens`, `--secrets`, `--passwords`), which were hidden when any part of the name matched.
- A quoted secret that starts `,"`, `,[`, `,{`, `:"`, `:[` or `:{` reads as JSON going on, and stays
  visible. One that starts with `}` or `]`, or with `,` or `:` and anything else, is hidden.
- Raw JSON with whitespace before a comma (`"--token " , "b"`) may not stay valid JSON.
- In raw stream fragments, as before this change: an environment value in escaped quotes
  (`API_KEY=\"x\"`) and a secret-named JSON key (`"password":"x"`, `"X-Api-Key":"x"`) are not hidden;
  only a frame redacted value by value has its keys checked.
- `-Password (ConvertTo-SecureString "x" …)` hides the cmdlet's name, not the value, as before.
- A hyphened header name with no space after its colon that doesn't start `X-` (`Api-Key:abc`).
- Hidden too much: prose such as `re-auth: needed now` reads as a header and loses `needed`; a path or a
  column after `--key-file`, `--sort-key`; `-Credential $cred`.

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
- **What ran on the tool's event, for `diag trace`.** Built first, then removed at review: it put command
  text into the event record and so into bundles, against 0011.
- **Cutting before hiding,** as the plan first had it: a key that straddles the 200th character would
  have lost the length that marks it as a key, and kept its start.
- **Trusting Codex's own `commandActions[].command`** for the script: it held the script for 403 of the
  541 commands and the whole wrapped line for the rest.
- **A backfill from `engine_raw`.** Code for one day's use, and the raw frames are to be pruned.

## Tests and their frames

`steps_claude.jsonl`, `steps_codex.jsonl` and `steps_shape_changed.jsonl` in `crates/gathr/tests/frames`
are **hand-written, not recordings**: made-up commands, paths, URLs and queries in the shape verified
above, each saying so in its first row. Nothing in them comes from a real conversation. The real
recordings (`background`, `approval`, `subagent`, `exits_mid_reply`) cover the tools they hold.

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
  `pwsh.exe -Command` on Windows (all 541 still come out of their shell under the command-flag rule). The
  same split handles `zsh -lc`, by the rule above, but no real frame proves it.
- **Codex's web search** rests on one real call.
- **How many steps a real turn has,** so the cost per turn.
