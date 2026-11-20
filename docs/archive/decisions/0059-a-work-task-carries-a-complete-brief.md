# 0059: A work task carries a complete brief, and its giver can edit it until it starts

Status: accepted (owner, 2026-11-15). Date: 2026-11-15.

Built: yes. Proof: `crates/gathr/tests/tasks/briefs.rs`, `crates/gathr/tests/edit_task.rs`, `crates/gathr/tests/tasks/words.rs`, `crates/gathr/tests/words.rs`, `crates/gathr/tests/tools.rs`, `crates/gathr/tests/tasks/review_notes.rs`, `ui/src/tasks/TaskReport.test.tsx`.

Amended by [0063](0063-building-waits-for-a-new-asks-plan.md): a task readied on a held ask waits for its plan, and `give_task` takes `look_into`.

Builds on [0010](0010-coordination-as-state.md), [0055](0055-a-members-task-starts-in-a-new-session.md) and [0056](0056-gathr-records-which-words-opened-each-turn.md). Serves promise 3 of [the charter](../charter.md#the-promises), parts 3.2 to 3.4 and 3.8 of [the truth map](../truth-map.md): a task stands on its own, and is corrected before it starts, not after. Stage 3's part D.

## The choice

- **D1: a work task is given complete.** `give_task` takes the kinds `work` and `review`; any other kind is refused with "isn't a kind: work or review".
  - A work task needs `why`, `done` (what is true when it's done), `proof` (how that is shown) and `where` (the folder and branch, or "no code"). A blank one counts as missing. The refusal names each one missing: "A work task needs why, done, proof and where. Missing: `done` and `where`. …".
  - `size` (the files or area it touches) and `lines` (the rough lines it adds) are optional and go together. One without the other is refused, and `lines` below 1 is refused. An edit checks the pair on the task as it would be saved.
  - A review takes none of `done`, `proof`, `where` or `size`: Gathr says what its done and proof are ([0053](0053-a-review-asks-both-questions.md)). One given is refused.
  - Gathr's own fix of a review's notes fills the four fields itself. Its `where` is the `where` of the PR's work task, else "PR #<n>'s branch".
  - The person's questions and members' requests aren't given through `give_task`, and take none of this.
  - `where` is one text. Nothing in Gathr reads it as a kind and a value, and Gathr doesn't check that the folder or branch exists.
- **D2: each limit is told before it is hit.** The limits are in the tool's JSON schema (`maxLength` 80 for `what`, 300 for `done` and `proof`, 200 for `where` and `size`; `minimum` 1 for `lines`), in each field's description and in the refusal: "`done` is 340 characters: cut 40 to fit 300, and put the details in `why`". `why` has no limit.
- **D3: the brief says every field it has.** A work task's brief gains, after `Why:`, the lines `Done when:`, `Proof:`, `Where:` and `Size: <size>, about <lines> lines`, each only when it is set. A review's and a request's brief keep their form. The briefing names `edit_task` among the tools.
- **D4: `edit_task`, until the first turn starts.** Only the agent that gave the task may change its `what`, `why`, `done`, `proof`, `where`, `size`, `lines` and `after`, and only what it names changes. An empty `size` clears the size and its lines. `after` replaces the list, and a loop is refused.
  - **Until it starts.** The task has started once the turn runner takes its first turn to send it: after the turn's login and usage checks, and before its engine starts, so a turn dropped then leaves no engine started for nothing. At that moment, in the same write that reads the turn's ask again, `started_ts` is set (`tasks::claim_ask`). The writer runs one job at a time, so an edit either lands before and is the text sent, or lands after and is refused as started. A turn whose engine then doesn't start, or refuses it, still counts: the task is started.
  - **Refused**, in this order, each as `edit_task failed: <why> (ref …)`: no such task in the chat; a question for the person; not its giver; started; not waiting, ready or blocked; an archived chat; the edited task fails D1 or D2; an `after` that is outside the chat, itself or a loop; nothing changes.
  - **What an edit does.** It writes the fields, when and which fields changed (`edited_ts`, `edited`), and one event `gathr.task.edit` with the old values. When `after` changed, the task is put back in line as `back_in_line` puts it: a ready task that now waits drops its queued turn, a waiting or blocked one that no longer waits becomes ready and queues its turn. A ready task's queued brief is rewritten in place, with the carry it was queued with, and its turn's recorded version follows. The window's task view and plan show the edit live; the chat doesn't draw a task's own brief (`ui/src/chat/messages.ts`), so nothing changes there.
  - **The dropped turn.** An edit drops a queued turn as a cancel does: it marks it (`turns.stop_why`), and the runner's gate drops a marked turn when it comes up, so it never runs. A marked turn's end never moves its task, and so tells its giver nothing: whoever marked it already put the task where it belongs. That holds whatever happened meanwhile: the task still waits or is blocked; it is ready again with a new turn (which alone runs, as the gate drops every marked one); it was cancelled; or its new turn started first. A turn dropped for another reason (an archived chat, a task that may not start) ends as it did before.
  - **The answer** is "Task #3 edited (what, done): ready.", "… waiting on #1, #2." or "… blocked (<reason>).". The fields are named as the tool names them.
- **D5: the window shows it.** `GET /api/chats/<id>/tasks` carries `done`, `proof`, `where`, `size`, `lines`, `edited_us` and `edited`, each left out when empty. The task view gains the rows "Done when", "Proof", "Where", "Size", "After" and "Edited", each only when set. The view stays read-only.
- **The nine columns on `tasks`** (migration `0045_task_briefs.sql`). All nullable.
  - `done`, `proof`, `place`, `size`: the fields. The column is `place` because `where` is an SQL keyword. The tool and the API say `where`.
  - `size_lines`: the number, apart from the words, because a later part's growth check reads it, and nothing reads a number out of words.
  - `started_ts`: when the first turn was taken to be sent. No query of `turns` tells a turn that ran from one that ended while still queued, so the moment is kept. The migration sets it for every task with a turn that ran or ended, and every task working, held, done, failed or stopped; when it started isn't kept, so its last change stands in. A blocked or cancelled task with no such turn isn't marked: it may be blocked by its `after`, or cancelled, without ever having run.
  - `carry`: the carry-on its queued brief was built with, so an edit rebuilds the same brief. A task can be queued with a carry and never have started (a restart after it was blocked for its `after`, an owner's report on a task still waiting, a merged PR).
  - `edited_ts` and `edited`: the last edit's time and fields, for the task view.
- **The kept samples of 0056.** The `brief` kind's version goes from 1 to 2: its three samples stay as they were, and "brief: complete" and "brief: size" are added. No other kind's version moves.

## Why

- A task given with only a title leaves the owner to guess what done means. A brief that must say how it is proved and where the work goes is one an agent with no memory of the chat can act on.
- A limit told only after the call (a refusal) costs a turn. In the schema, an agent sees it first.
- A giver finds a wrong word after giving. Cancelling and giving again loses the task's number and its place in line; an edit before it starts costs nothing.

## Turned down

- **`where` as a kind plus a value:** nothing reads it as one.
- **Gathr filling `where` from the owner's worktree.** The briefing already names that folder. What the giver knows and Gathr doesn't is which branch or PR the work belongs on.
- **The required fields as required in the schema.** The library would refuse a call before Gathr saw it, with no reference and without naming every field missing.
- **Checking the fields' words.**
- **A limit on `why`.**
- **Moving an over-long title into `why` by itself.** Gathr would guess where the title ends. The limit in the schema and the refusal saying to put the rest in `why` are what end it.
- **Editing after the first turn starts.** The owner may be acting on it. A message reaches a running turn.
- **The person editing in the window.**
- **Editing questions for the person.**
- **Stopping a queued turn and queuing a new one on an edit, and reading the ask again at the gate.** The new turn would drop a teammate's message joined to the old one, and a stopped ready task ends as stopped. Marking started at the gate would refuse every edit while a turn waits for a sign-in or a usage limit.
- **Refusing an edit once a turn on the task ended, in place of the `carry` column.** A restart after a block, an owner's report on a task not started and a merged PR all queue a carry with no turn started or ended.
- **Showing old values in the window.** They are in the event.
