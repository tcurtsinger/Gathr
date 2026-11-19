# 0085. Context after a compaction

Status: accepted
Built: partly — `crates/gathr/tests/sessions/compacted.rs` proves the plan and open tasks return for the
member's restarted task and the lead's no-task turn. Role retention is unverified on both engines.

Promise 8.4: the first turn after a compaction receives the plan's version and state,
the part its task belongs to, its open tasks in list_tasks' words, and the whole task
record. Restore these from the service's records rather than asking a summary to keep them.
Ordinary turns keep their existing opening.

A pending restoration belongs to the conversation: `task_sessions` and `engine_sessions`
keep its marker alongside the provider session id and account. A different task's turn cannot
take it, and a service restart cannot lose it. The first opening of that conversation clears
all rows pointing to it. Replacing a row with another session clears its old marker.

The service seam in `tests/sessions/compacted.rs` proves what the test engine receives after
compaction, using the sessions tests' task/conversation setup. `tests/context.rs` covers compaction
itself. The sessions root is near the file limit, so the new scenario lives in its own file.
It cannot prove a provider preserves its system briefing. Each provider needs a separate
check with a word placed only in the briefing, never spoken before compacting. Role retention
remains unverified for Claude Code and Codex: the person skipped the live check after sign-in
failed in the dev window. No made-up-word transcript was produced. Promise 8.4 remains Partly.

Rejected: trusting the conversation summary to retain task records or the role. Those are
the facts this promise exists to restore.
