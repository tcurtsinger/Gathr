# 0084: A task's turn opens with its questions, and what came since its conversation last ran

Status: accepted (owner, 2026-11-19: approved plan "Stage 4's rest: what a turn opens with", version 2, part 3).
Date: 2026-11-19.

Built: yes. Proof: `crates/gathr/tests/sessions/record.rs`.

Amends [0081](0081-agents-hear-the-room-and-name-each-other.md) (since when a turn hears the chat's lines) and builds
on [0055](0055-a-members-task-starts-in-a-new-session.md) (a member's task has its own conversation) and
[0065](0065-questions-in-the-leads-words.md) (a member's question reaches the person through its lead). Serves
promise 8 of [the charter](../charter.md#the-promises), parts 8.1 and 8.2 of [the truth map](../truth-map.md).

## Why

A question to the person is a person task, tied to the turn that asked it (`given_in`); a member's question is filed
by its lead for the member's request (`request`). The answer opened one `answered` turn, or the member's hear-back,
and after that nothing brought it back to the task's conversation: a later turn on the task, a new conversation for
it, or one the agent came back to from another task, had to look it up. 0081 had the same gap for the chat's lines:
a member back on task A after task B heard what came since its last turn on B, and missed what was said between.

## The choice

- **One "since" for both.** A turn's conversation last ran at the newest started turn of the agent in the chat that
  ran in it (`room::last_ran`). Each turn keeps the conversation it ran in (`turns.session_id`, migration 0062): the
  open engine's session as the turn starts, or the one its engine reports at Init. When the engine opens again on a
  task's conversation (0055's resume), that is the newest turn that ran in it, whatever task it was on: a turn kept in
  another conversation under a background command (0055's N4) doesn't count. Otherwise the engine is already open, or
  opened on the agent's latest conversation, and ran the agent's last turn there. A new conversation, or one a
  compaction summed up, has no "since": it hears the last 30 lines and every question. Turns from before the
  migration have no conversation, so the first resume after it hears the last 30 lines.
- **The chat's lines** (0081) count from it. Coming back to a task's conversation, the agent also hears its own
  lines said since in other conversations, which that one never had; still the last 30 at most.
- **The questions.** A turn on task T (`turns.task`, so the lead's task turns too, though they run on its one
  conversation) opens, after the chat's lines and before its ask, with "Questions to the person from task #T:" and
  one line for each question asked from T, by T's owner (its `given_in` turn was on T) or by the lead for a request
  asked from T (the request's `given_in` turn was on T): `- #<n> <what>: <answer>`, the answer as agents hear it
  ("Picked: … Their words: …"), "cancelled" with its reason, or "not answered yet". Only those asked, or answered
  or cancelled, since the conversation last ran; all of them in a new one.
- **Never told twice.** Each turn that carries an answer records which question's (`turn_answers`, migration 0062),
  and its opening leaves that question out: the `answered` turn, and each brief readied when a task blocked on the
  question is let go. A request's hear-back, or a brief readied by it, carries the question that settled it
  (`tasks.settled_by`) and no other: the request's other questions, such as one a Discuss replaced or a second answered
  in the same batch, still show. `heard_in` stays the turn the answer was heard in, which the chat places it by.
- **Diagnosable.** A turn that opens with questions records `gathr.task.questions` with how many; a failed read is
  `gathr.room.heard`'s error, and the turn goes without both.

## Turned down

- **The task's `task_sessions` row as the history of where its turns ran.** The row holds only the task's latest
  conversation. A turn kept in another under a background command (N4) would count for the task, and when it went back
  to its own, lines and questions said in between would be missed, not just heard twice.
- **`heard_in` as the turn that carries an answer.** The chat places an answer by it, so a brief may not move it, and
  one link can't name several briefs, or tell which of a request's questions a turn carried.
- **The questions in a task's brief.** The brief is the task's first ask; the questions come after it, and a turn
  that isn't the brief (a wait, a hear-back) needs them as much.
- **Every question every turn.** A long task would carry every old answer into every turn, where the conversation
  already has them.
