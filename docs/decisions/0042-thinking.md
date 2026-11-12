# 0042. A turn shows that its agent thought, as a time

Status: accepted (owner, 2026-11-11, on the plan for the chat redesign's fifth slice; Codex's part
on 2026-11-12). Date: 2026-11-12. It ends 0037's line "Steps show no thinking until the service
keeps it".

## Choice

- **A thought is a message of kind `thinking`, with no text.** Its body is JSON, `{"status",
  "duration_ms", "tokens"}`: `running`, then `done` or `stopped`; how long it took by Gathr's clock;
  and the engine's own estimate of its tokens when it gives one. No thinking text is read, kept or
  shown: none arrives (below).
- **The window shows it as a step of the turn's row:** "Thought for 9s", in the row's own duration
  format, with nothing to open and no chip. A thought under a second is left out. Thoughts are
  listed in order with the steps and are not counted in "6 steps": that count is what the turn ran.
  A turn whose only work was a thought under a second has no row.
- **Claude Code says when a thought starts and ends, so its row says "Thinking" while one runs**
  (shimmer, no spinner, as every running row). The driver reads three frames:
  - a `content_block_start` whose block is `thinking` or `redacted_thinking`, and
    `system/thinking_tokens` (`estimated_tokens`), whichever comes first: the thought is under way.
    Each later `thinking_tokens` brings the count so far; the last one is kept as `tokens`.
  - the `assistant` frame that carries the block (empty, with a signature): the thought is over. In
    the recordings it arrives before the block's `content_block_stop` and before the reply's text
    starts, so it is the earliest end there is.
  The message is made at the start (`running`) and closed at the end, as a tool's is: live, a
  `message` event, then the `tool` event that carries a message's new body. A thought still open
  when words or a tool follow is closed there, so a count that arrived late never leaves the row
  saying "Thinking" over a reply.
- **Codex says only that it thought, once it has, so nothing shows live and the time is the quiet
  before the mark.** A `reasoning` item's `item/started` and `item/completed` arrive together when
  the thinking is over. The driver reads the `item/completed` as one mark. The service makes a
  message, already `done`, that lasted from the turn's last sign of progress (its start, a step's
  start or end, the last streamed text, the mark before) to the mark, and only when no step is
  running and that is a second or more. Its time is when the quiet began, as a tool's time is when
  it began, never before the turn's last message. A mark while a command runs, or right after
  another mark, makes nothing. While Codex thinks, its row keeps its working label.
- **Both engines go through the same two frames,** `Thinking { tokens }` and `Thought`. A `Thought`
  with a thought open closes it; with none open it is a mark. The service holds what is open, with
  the turn; Claude Code's reader stays a function of one frame.
- **Thinking is progress, never a reply.** It refreshes the turn's last progress, so a long thought
  is not flagged as a stall (G1, G8, G34), and it does not count as having said anything: a turn
  that only thought still fails as empty (G167).
- **A sub-agent's thinking stays under its Agent call** and is never the agent's own (G24).
- **A turn cut off mid-thought keeps the thought, `stopped`,** with the time it ran, as a tool is.
- **A thinking message is never words.** Search indexes text messages only, `read_chat` reads text
  messages only, and nothing that builds what an engine is told reads the chat's other kinds. A
  thought counts in a chat's message count and as activity, as a tool call does.
- **Schema:** migration `0028` rebuilds `messages`, as SQLite changes a CHECK only that way. Every
  message keeps its rowid, `to_agent` and `copy_of`; the index and the three search triggers are
  made again; `message_search` is emptied and filled from the text rows.
- **Found from Diagnostics** (0019): one `gathr.turn.thinking` event per thought kept, on the turn's
  trace, with its duration and `gathr.thinking.tokens_estimate` when the engine gave a count; a
  thought cut off is that event with error `stopped`. No text exists to copy.

## What was checked

- **Claude Code's recordings** (2.1.318, Gathr's own flags): `steered_after_turn.jsonl` (Opus 5.5)
  holds one thinking block, its `thinking_delta` empty with `estimated_tokens: null`, a signature,
  and the `assistant` frame's block `{thinking: "", signature}`. `background.jsonl` (Haiku 5.5) holds
  two `system/thinking_tokens` frames, 50 then 204, then the same empty block; it has no stream
  events for that turn. No recording holds `redacted_thinking`, and none holds any thinking text.
- **Claude's docs** (read 2026-11-12: code.claude.com agent-sdk/typescript, headless, vs-code;
  platform.claude.com build-with-claude/thinking): `system/thinking_tokens` is documented from
  Claude Code 2.1.153, sent while a thinking block, a redacted one too, is produced, with
  `estimated_tokens` and `estimated_tokens_delta`, for progress display, and is not tied to a
  model. The VS Code extension shows a thinking block as "Thought for 12s" with no text. `display`
  is `omitted` by default on every Claude 5 model: the block arrives empty with a signature.
- **Codex in real use** (the owner's own Gathr data, read on 2026-11-12 with the owner's leave,
  read-only, for structure and counts only): 505 `reasoning` items in 82 turns, each
  `{type, id, summary: [], content: [], startedAtMs, completedAtMs}`, never any text, up to 112 in
  one turn. Each item's own times are under 100 ms apart, so they are no duration. The quiet before
  a mark is: median 6.0 s, 90% under 20 s. 74 marks follow another mark directly and 53 arrive
  while a command is still running.
- **Codex's app-server docs** (read 2026-11-12: developers.openai.com/codex/app-server, which
  redirects to learn.chatgpt.com/docs/app-server, and the config reference): a `reasoning` item is
  `{id, summary, content}`; `item/reasoning/summaryTextDelta`, `summaryPartAdded` and `textDelta`
  stream while the item is open; `turn/start` takes a per-turn `summary` (the example value is
  "concise"; the config key `model_reasoning_summary` takes `auto`, `concise`, `detailed` or
  `none`). No default is documented. Gathr sends no `summary` on `turn/start`
  (`codex/rpc.rs`), which fits the empty, instant marks seen in real use. SerenityOS sends none
  either.

## Why

- Work that shows no words still shows as work (S1, G1, G8), and silence is told apart from
  thinking (G34), with the only thing verified to arrive: that it thought, and for how long.
- The owner's call: "Thinking" live and "Thought for 9s" after, no text, for both engines; then,
  with Codex's facts in hand, nothing live for Codex.
- One kind of message keeps a thought out of every place that reads words or tool calls, without
  each of them learning a made-up tool name.

## Turned down

- **Thinking text or summaries.** None arrives on the models Gathr runs. A summary may be switched
  on (`showThinkingSummaries`, Codex's `summary`), but neither is verified for Gathr's runs, and
  the owner gets a second question, with real sizes, before any text is kept.
- **A `tool` message named "thinking".** No migration, but every reader of tool messages would have
  to know the fake name, and an engine may ship a tool called that.
- **A live "Thinking" label for Codex.** It has no start signal, so the label would be a guess
  until the mark arrives.
- **Codex's own item times** as the duration: under 100 ms each, whatever the thinking took.
- **A Codex mark timed while a step runs,** or kept under a second: the quiet then is the command's
  time, or nothing.
- **Tokens on the end frame,** as the plan first had it. The count comes during the thought, and
  the reader of one frame keeps nothing between frames; the service keeps it with the open thought.
- **Counting thoughts in "6 steps".** Codex marks up to 112 times a turn; the count says what the
  turn ran.

## Tests and their frames

`thinking_claude.jsonl`, `thinking_cut_off.jsonl`, `thinking_only.jsonl` and `thinking_codex.jsonl`
in `crates/gathr/tests/frames` are **hand-written, not recordings**, in the shapes above, each
saying so in its first row. Nothing in them comes from a real conversation. `thinking_codex` holds
`sleep` rows: the quiet a mark is timed by is real time, 1.1 s twice.

## Cost

A body is 17 to 50 bytes (`{"duration_ms":9400,"status":"done","tokens":204}`). The signature
(about 1,000 characters a thought) stays only in `engine_raw`, as before. A Codex mark that is kept
costs one read of the turn's last message and one insert; one that is not costs nothing. The
rebuild copies `messages` once, at the first start after the update: measured on made-up data
(58,500 messages in 300 chats, 29 MB of text), 0.55 s for the migration's own statements and 0.7 s
through a debug build.

## Not built, not verified

- **Whether `showThinkingSummaries` does anything for a headless (`-p`) run:** the docs cover
  interactive sessions. Not verified.
- **Whether `system/thinking_tokens` needs `--include-partial-messages`:** Gathr passes it, and
  `background.jsonl` holds the frames with no stream events around them, but which flags that
  recording ran with was not checked. Not verified.
- **What order `thinking_tokens` and the block's start arrive in when both do,** and whether a
  count can arrive after the block: no recording holds both. Either order opens one thought.
- **Whether sending `summary` on Codex's `turn/start` makes `item/started` arrive when reasoning
  begins,** which would give Codex a real start, a live "Thinking" label and a real duration. The
  docs say only that `item/started` fires "when a new unit of work begins". It needs one short run
  on a signed-in Codex test account, which nobody in this build had. Until then Codex's duration is
  the quiet before the mark, and it has no live label.
- **`redacted_thinking`:** read the same as `thinking`, from the docs; no real frame of it was
  seen.
- **How long the rebuild takes on a person's real database.**
