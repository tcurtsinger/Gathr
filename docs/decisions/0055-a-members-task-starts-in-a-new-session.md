# 0055: A member's task starts in a new session

Status: accepted (owner, 2026-11-14); amended by [0081](0081-agents-hear-the-room-and-name-each-other.md): the new conversation also hears the chat's last 30 lines, and its opening says so. Date: 2026-11-14.

Built: yes. Proof: `crates/gathr/tests/sessions.rs`, `crates/gathr/tests/tasks/sessions.rs`, `crates/gathr/tests/archive.rs`.

Builds on [0010](0010-coordination-as-state.md#sessions) and [0020](0020-context-per-turn.md). Serves promise 8 of [the charter](../charter.md#the-promises), part 8.8 of [the truth map](../truth-map.md): a new session picks a task up and loses nothing. Stage 4's first slice; it comes ahead of Stage 3 on the owner's word.

## The choice

A member's task gets its own conversation with its engine. Nothing is set by hand. The lead keeps one conversation per chat.

- **N1: every turn gets a key when it starts.** `gathr::sessions::want` reads it from the turn's row.
  - The chat's Supervisor: `Lead` (the same as Latest in what it does; it is told apart for `gathr.session.for`). A turn with no agent: Latest.
  - A turn with a task: that task.
  - A turn with no task that answers a turn of the same agent that had one: that task. This covers a hear-back after the giver's task closed, the answer to a person task the agent asked, and a carry-on.
  - Anything else: Latest, the agent's latest conversation, as before. That is a teammate's message (even one naming `#N`), the person's message, a wake, a compaction, a wait left in a turn with no task, and the landing hand-off.
- **N2: the key gives the conversation** (`gathr::sessions::choose`).
  - Latest: the `engine_sessions` row, resumed or new, as before.
  - A task with a `task_sessions` row for this agent on the turn's account: resume it. A session resumes only in the login folder it was made in, so the row keeps its account.
  - A task taken from a turn it answers, with no usable row: Latest. A hear-back never opens a new conversation.
  - A fix task Gathr gave (named in `review_notes.fix`): it resumes the row of the PR's work task (`notes::work_on`) when that task is the same agent's, on the same account and isn't the fix itself. Otherwise New.
  - Anything else: New.
- **N3: switching.** `start` compares the conversation the turn wants with the one the running engine was opened in. The engine's `Session` (`turns/session.rs`) keeps the id it resumed, or the one its first Init reported; it never reads a later Init's id. A switch happens when an engine is running and the turn gets New, or a resume of another id. The engine closes the gentle way, with the reason `the next turn is on another task`, and the turn stays first in the queue as `queued`. Latest never closes the engine.
- **N4: no switch under a background command.** Closing would end it. The turn runs in the conversation that is open, and Gathr records `gathr.session.kept`. If the turn would have opened New, the task's row takes the open conversation's id, so its later turns come back to it. A resumed task's row is left as it is.
- **N5: recording.** Each Init writes `engine_sessions` as before and, when the engine was opened for a task, that task's `task_sessions` row. A row that gets another id starts with no saved fill and untold.
- **N6: the opening.** A new conversation's first turn is sent `words::opening` before its ask. It isn't saved as a message. It says the conversation is new for the task, that `read_chat` and `list_tasks` have the earlier turns, and lists up to five of the agent's other tasks in the chat, newest first, each as `words::line`. If the ask isn't already the task's brief (a task that lost its conversation to N8), the brief follows.
- **N7: the context follows the conversation.** `task_sessions` keeps `used` and `told`. Saving a fill and the notice that a conversation is near its compaction point also write the row of the task the engine was opened for, so both are right however the engine later closes. A new conversation shows no fill until its first call. A resumed one shows its saved fill and isn't told again before it compacts. Latest changes nothing.
- **N8: a resume the engine refuses.** A start that resumed an id and ended its turn failed before any Init, with no refused sign-in, deletes that agent's rows holding the id (both tables), records `gathr.engine.resume_failed` on the turn's trace, closes the engine the gentle way with the reason `it couldn't resume its conversation` (an engine Codex's driver already closed keeps that close's reason, `the engine rejected its request`), and puts the turn back first in the queue, once. Nothing is written into the refused engine: once its close ends, the next start finds no row and opens New with the brief. A second failure fails the turn as before. This is one function, `session::refused`, called from the three places a refused resume can end a turn: a Codex request error, a Claude exit after the ask was written, and a failed write to a Claude process that already exited.
- **N9: delete and archive.** Deleting a chat deletes its `task_sessions` rows. Archiving keeps them, as it keeps `engine_sessions`. The engines' own stores aren't touched.
- **N10: events.** `gathr.engine.start` gains `gathr.task.id`, `gathr.session.for` (`task`, `latest`, `lead`) and `gathr.session.from` (`new`, `task`, `work_task`, `latest`). `gathr.engine.close` gains the reason `it couldn't resume its conversation`. New: `gathr.session.kept`, `gathr.session.opening` (`gathr.task.id`, `gathr.task.count`, `gathr.message.chars`) and `gathr.engine.resume_failed`.
- **N11: no setting.** To turn it off, revert the change, or make `want` return Latest. The table stays, unread.
- **Migration 0042** adds `task_sessions` (primary key task and agent) and gives each open task that already ran the session its owner has, so a task under way stays where it is.

## Why

- A member carries every task it has done in one growing conversation, so each call reads all of them again. Most of a member's input tokens are that, mostly as cache reads, and the conversation grows until it compacts. A task needs its ask and a short list of the recent ones, and `read_chat` and `list_tasks` have the rest.
- The saving is measured from Gathr's own events, not assumed. Before building, the figure came from the recorded turns; after it lands, the same query over a few days of use says whether it held (input tokens per member task turn, estimated context per call, cache writes, resume failures, quality no worse, time to switch).
- Going back to a task's own conversation keeps what that task learned, so a restart, a hear-back or a fix doesn't start cold.
- The key is read from the turn's own row, not guessed from timing or words ([0010](0010-coordination-as-state.md)).
- The choice stays out of `Assignment`: any difference in it closes the engine with no check for a background command, and N4 needs one.
- Both engines keep their session id when resumed, in the record read before building, so the comparison against the id the engine was opened with holds and no row has to follow an id that moved.

## Turned down

- **One conversation per task for the lead too.** The owner's answer: the lead keeps one per chat. Its conversation per plan stays in Stage 4.
- **A cap of three set-aside conversations** ([0010](0010-coordination-as-state.md#sessions) as first written). Nothing needs it yet.
- **Deleting old engine sessions.** Not needed; whether the engines prune them is not verified.
- **Several Codex threads in one app-server, or switching inside a running Claude process.** Neither is verified to exist.
- **A session column on `turns`.** The key is read from the row when the turn starts.
- **A setting.** See N11.
- **Moving the landing hand-off to the review's conversation.** None of the owner's answers asked for it, and its effect is unchecked. It runs on Latest, as before.
- **The room's last 30 lines in the opening.** The opening carries the last five tasks only.
- **Routing a teammate's free-text `#N` to that task's conversation.** It runs on Latest.

## Left for later in Stage 4

The lead's conversation per plan; the task's full record in the opening, and what a hear-back, a wait or a PR holds; what is given back after a compaction; routing a teammate's `#N`; standing rules and catch-up ([0020](0020-context-per-turn.md)); carrying on after a restart (part 8.5); a wake that says its age; a wait with no task firing where it was left; scenario 3d's whole brief.

## Different from the approved detail

- `want` gives the Supervisor's turns `Lead`, not Latest, so `gathr.session.for = lead` can be told apart. It acts as Latest.
- A task row on another account: the next Init moves it to the new account and id, so the turn opens New. A turn that answers another turn finds no usable row and runs on Latest.
- A fix needs its work task to be another task than itself: when the PR's first work task was cancelled, `work_on` can return the fix.
- On `gathr.engine.start`, `gathr.task.id` is the task the engine was opened for. For Latest it is empty, even when the turn has a task.
- The service writes the row's fill and notice in the same transaction as `context::tell_soon`, `untold` and `filled`, through `sessions::told` and `sessions::used`; those context functions are unchanged. `context::switched` is the one new context function.
- `sessions::set` on a row with another session id resets its fill and notice; the same id keeps both.
- A turn that ended signed out is never taken for a refused resume: a refused login doesn't delete a conversation.
- A resumed conversation's fill comes from the row that holds its session: the task's own when it has a fill, else the newest task's on that session. So a fix that resumes its work task's conversation shows that task's fill before the fix has a row of its own.
- If the engine has not reported an Init yet, any resume counts as another conversation and switches.
- If reading the key or the row fails, the turn runs on Latest, and the failure is recorded as `gathr.engine.session` with the error `sqlite`, as a failed write of a row is.
- A second refusal of the same turn isn't reached by a test: after the first, the rows holding the id are gone, so the retry opens New. A double requeue is not caught by any test.

## Changes

- `crates/gathr/src/sessions.rs`, `crates/gathr/src/bin/gathrd/turns/session.rs`, `crates/gathr/migrations/0042_task_sessions.sql`.
- `crates/gathr/src/tasks/words.rs` (`opening`), `crates/gathr/src/context.rs` (`switched`), `crates/gathr/src/archive.rs`.
- `crates/gathr/src/bin/gathrd/turns/` (`mod.rs`, `process.rs`, `reading.rs`, `context.rs`).
- The replay stand-ins: `GATHR_REPLAY_SESSION=echo` and `GATHR_REPLAY_RESUME=missing` (the stand-in's header in `crates/gathr/examples/replay_engine.rs` describes them).
