# 0042. A turn shows that its agent thought, as a time

Status: accepted (owner, 2026-11-11, on the plan for the chat redesign's fifth slice; Codex's part
on 2026-11-12). Date: 2026-11-12. It ends 0037's line "Steps show no thinking until the service
keeps it". Amended 2026-11-12: Codex is asked for a reasoning summary, which makes it say when a
thought starts, so its row says "Thinking" live too.

Built: yes. Proof: `crates/gathr/tests/thinking.rs`, `ui/src/chat/messages.test.ts`.

## Choice

- **A thought is a message of kind `thinking`, with no text.** Its body is JSON, `{"status",
  "duration_ms", "tokens"}`: `running`, then `done` or `stopped`; how long it took by Gathr's clock;
  and the engine's own estimate of its tokens when it gives one. No thinking text is read, kept or
  shown: none arrives, and Codex's summary of it, which does, is dropped (below).
- **The window shows it as a step of the turn's row:** "Thought for 9s", in the row's own duration
  format, with nothing to open and no chip. A thought under a second is left out. Thoughts are
  listed in order with the steps and are not counted in "6 steps": that count is what the turn ran.
  A turn whose only work was a thought under a second has no row. Thoughts with nothing between
  them show as one step, with their time together (the owner's call, 2026-11-12: a real Codex
  turn thought in four pieces back to back, and read as a list). The messages stay one a thought.
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
- **Codex is asked for a reasoning summary, and then says when a thought starts and ends too.**
  Gathr sends `"summary": "auto"` on every `turn/start`. A `reasoning` item's `item/started` then
  arrives when the thinking begins and its `item/completed` when it ends: the driver reads the
  first as `Thinking` and the second as `Thought`, and the thought opens and closes by Gathr's
  clock, as Claude Code's does. The summary is what was asked for, not what is wanted: its frames
  (`item/reasoning/summaryPartAdded`, `summaryTextDelta`, `textDelta`) and the `summary` in the
  completed item are never read into a frame, a message, an event or the API. They stay in the
  raw frame record only (Cost, below).
- **A Codex start is held a quarter of a second, because without a summary it is no start.** A
  model or an engine that gives no summary sends `item/started` and `item/completed` together,
  once the thinking is over. So the service holds a Codex `Thinking` for 250 ms before it opens
  the thought. An end that comes within that is a mark: the service makes a message, already
  `done`, that lasted from the turn's last sign of progress (its start, a step's start or end,
  the last streamed text, the mark before) to the mark, and only when no step is running and
  that is a second or more. Its time is when the quiet began, as a tool's time is when it began,
  never before the turn's last message: moved later for that, it is as much shorter, and dropped
  if under a second then. A mark while a command runs, or right after another mark, makes
  nothing. With no end in that time the thought opens, timed from when its start came. Words or
  a tool within it drop the held start. When the wait ends with a line already read from the
  engine and not yet handled, that line goes first, as it may be the end.
- **Both engines go through the same two frames,** `Thinking { tokens }` and `Thought`. A `Thought`
  with a thought open closes it; with none open it is a mark. The service holds what is open, and
  a Codex start it is holding, with the turn; each reader stays a function of one frame.
- **Thinking is progress, never a reply.** It refreshes the turn's last progress, so a long thought
  is not flagged as a stall (G1, G8, G34), and it does not count as having said anything: a turn
  that only thought still fails as empty (G167).
- **A sub-agent's thinking stays under its Agent call** and is never the agent's own (G24).
- **A turn cut off mid-thought keeps the thought, `stopped`,** with the time it ran, as a tool is.
  When the service itself died mid-turn, the next start fails the turn and stops the steps it left
  `running`, tools and thoughts alike; how long such a thought ran is not known, so it has no
  duration and the window leaves it out.
- **No thought opens while one of the agent's own tools runs.** A count that arrives then may be a
  sub-agent's, or one that came after its block, and nothing would close it until the next words
  or tool: a made-up "Thought for 3m". It counts as progress and nothing more.
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
  `none`). No default is documented. Until this was amended Gathr sent no `summary` on
  `turn/start`, which fits the empty, instant marks seen in real use. SerenityOS sends none.
- **Codex with and without a summary** (2026-11-12, `codex app-server` 0.167.2, a signed-in test
  account, one puzzle that needs no tools, three runs: none, `auto` and `detailed`):
  - no `summary`: reasoning items at 22, 40, 54, 69 and 73 s, each `item/started` and
    `item/completed` 1 to 4 ms apart, `summary: []`, `content: []`.
  - `"summary": "auto"`: `item/started` when the thinking begins (3.3 s) and `item/completed`
    when it ends (24.3 s); the next item starts within 5 ms (24.3 to 40.0 s, 40.0 to 53.5 s),
    then the answer. `"detailed"` read the same way, in two items. In between, one `summaryPartAdded` and one
    `summaryTextDelta` of 29 to 38 characters per part, every 3 to 12 s, the first 10 to 12 s
    after the start. The completed item's `summary` holds 1 or 2 strings; `content` stays `[]`.
    The item's own `startedAtMs` and `completedAtMs` are then the thought's real times.
- **Codex's source** (github.com/openai/codex at tag `rust-v0.167.2`, read 2026-11-12): `summary`
  on `turn/start` is "for this turn and subsequent turns" (`TurnStartParams.ts`) and is passed on
  as a settings update with no check against the model (`app-server/.../turn_processor.rs`,
  `core/src/session/step_settings.rs`). With none given, the model's own
  `default_reasoning_summary` is used. It is left out of the request to the model, quietly, when
  the provider has no reasoning summaries, the model's `supports_reasoning_summary_parameter` is
  false, or it is `none` (`core/src/client.rs`, `build_reasoning`). So a model that takes no
  summary is expected to run as before, with instant marks. Read, not run.
- **The raw frame record** (0011) keeps every frame's body, redacted for secrets only, so it
  holds a summary's text as it holds an agent message's: checked by replaying
  `thinking_codex_live` and reading `gathr diag turn <ref> --raw`. A `summaryTextDelta` is held
  and redacted joined with its item's other pieces, as an `agentMessage/delta` is.

## Why

- Work that shows no words still shows as work (S1, G1, G8), and silence is told apart from
  thinking (G34), with the only thing verified to arrive: that it thought, and for how long.
- The owner's call: "Thinking" live and "Thought for 9s" after, no text, for both engines; then,
  with Codex's facts in hand, nothing live for Codex; then, once a run showed that a summary
  gives Codex a real start, live for Codex too, which is what was first asked for.
- A real start beats the quiet before a mark: the quiet counts the wait for the model as thought,
  and shows nothing until the thought is over.
- One kind of message keeps a thought out of every place that reads words or tool calls, without
  each of them learning a made-up tool name.

## Turned down

- **Thinking text or summaries.** No thinking text arrives on the models Gathr runs. Codex's
  summary now arrives, as the price of its start, and is dropped; Claude Code's
  (`showThinkingSummaries`) is not verified for Gathr's runs. The owner gets a second question,
  with real sizes, before any text is kept.
- **A `tool` message named "thinking".** No migration, but every reader of tool messages would have
  to know the fake name, and an engine may ship a tool called that.
- **A live "Thinking" label for Codex with no summary asked for.** It has no start signal then,
  so the label would be a guess until the mark arrives.
- **Opening a Codex thought at once and fixing it up if its end comes with it.** The message is
  already saved and sent to the window by then: a mark that should make nothing (up to 112 a
  turn) would leave a row each, and one that is kept would have to move in time.
- **Holding the start in the driver until the next frame.** With a summary the next frame is 10 s
  or more away, so the label and the duration would both start late.
- **Telling a mark by the item's own times.** They are only known at `item/completed`, after the
  thought must already show.
- **`"concise"` or `"detailed"`.** `auto` leaves the choice to the model; nothing here reads the
  summary, so the smallest one that still gives a start is wanted, and which that is was not
  measured.
- **Codex's own item times** as the duration: without a summary, under 100 ms each, whatever the
  thinking took; with one they are real, but Gathr's clock already times every other step.
- **A Codex mark timed while a step runs,** or kept under a second: the quiet then is the command's
  time, or nothing.
- **Tokens on the end frame,** as the plan first had it. The count comes during the thought, and
  the reader of one frame keeps nothing between frames; the service keeps it with the open thought.
- **Counting thoughts in "6 steps".** Codex marks up to 112 times a turn; the count says what the
  turn ran.

## Tests and their frames

`thinking_claude.jsonl`, `thinking_cut_off.jsonl`, `thinking_only.jsonl`, `thinking_in_a_step.jsonl`,
`thinking_codex.jsonl` and `thinking_codex_live.jsonl`
in `crates/gathr/tests/frames` are **hand-written, not recordings**, in the shapes above, each
saying so in its first row. Nothing in them comes from a real conversation. `thinking_codex` holds
`sleep` rows: the quiet a mark is timed by is real time, 1.1 s twice. The second begins at a
`hold` row, let go when the test hears the command end: the service counts that quiet from when
it handled the end, so a sleep begun when the engine wrote it is counted short by however far
behind the service was (1.1 s as 944 ms once, on Windows in CI). `thinking_codex_live` holds
two thoughts with summaries, each a `sleep` of 1.2 s long; its test also fails if a summary's
words reach the chat's history, a live event or the turn's trace.

## Cost

A body is 17 to 50 bytes (`{"duration_ms":9400,"status":"done","tokens":204}`). The signature
(about 1,000 characters a thought) stays only in `engine_raw`, as before. A Codex mark that is kept
costs one read of the turn's last message and one insert; one that is not costs nothing. A Codex
thought under way costs an insert and an update, as Claude Code's, and its label shows 250 ms
late. A summary adds two frames a part to `engine_raw`, with its text (about 30 characters a
part in the run above), kept as long as raw frames are. What a summary costs in tokens was not
measured. The
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
- **A Codex turn with tools, with a summary on.** The run had none. Whether a reasoning item can
  still be open when a command starts, and what order its end and the command's start come in,
  is not verified; words or a tool close an open thought either way.
- **Other Codex models.** One model was run. One that gives no summary is expected to send
  instant marks (the source, above), which the held start reads as before. Not run.
- **What a summary costs in tokens,** and whether `"concise"` costs less than `"auto"`. Not
  measured; `"concise"` was not run.
- **A Codex thought shorter than the 250 ms hold** reads as a mark, so it is timed by the quiet
  before it, or dropped. None that short was seen with a summary on.
- **A Codex thought over the stall time (10 minutes) whose summary stops coming.** Its start and
  each of the summary's frames count as progress (the frames came every 3 to 12 s in the runs),
  so a long thought is not flagged. One that sent none for the stall time would be flagged once
  as showing no progress, never stopped. None was seen.
- **A real Codex turn through the service,** 2026-11-12, the same account and puzzle: the row
  read "Thinking" 4 s in, and the turn kept four thoughts of 21, 17, 10 and 14 s, back to back.
  A second turn that ran two commands in 13 s kept no thought.
- **Whether `summary` stays set on a resumed thread.** Gathr sends it on every `turn/start`, so
  it does not rely on it.
- **Whether Claude Code puts `parent_tool_use_id` on a sub-agent's `system/thinking_tokens`.**
  The reader keeps it under the Agent call when it does; when it doesn't, the rule above (no
  thought while a tool runs) keeps it out.
- **Codex's own waits to reconnect** don't count as progress, so the thought after one may include
  the wait in its time.
- **`redacted_thinking`:** read the same as `thinking`, from the docs; no real frame of it was
  seen.
- **How long the rebuild takes on a person's real database.**
