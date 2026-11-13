# 0048: Gathr records friction, starting with what agents report

Status: accepted (owner, 2026-11-12). Date: 2026-11-13.

Built: partly. Proof: `crates/gathr/tests/coordination.rs`, `ui/src/settings/Friction.test.tsx`. Not built: the person marking friction of their own; Gathr keeping the ledger that says what each entry needs (both with the rest of promise 11).

Serves promise 11 of [the charter](../charter.md#the-promises): no rulebook needed, and Gathr measures it (part 11.4 of [the truth map](../truth-map.md)).

## The choice

- **A tool for every agent, from any turn:** `report_friction`, with `what` (what got in the way, in a sentence or
  two, at most 500 characters) and `where` (the tool, screen, command or file; optional).
- **Gathr keeps it** in its own table, with the agent, the chat, the turn, the turn's task and the time, and records
  an event (`gathr.friction.report`). A report outlives its chat and its agent.
- **It is read in three places:** Settings > Diagnostics (a Friction list under the turns), `GET /api/friction`, and
  `gathr diag friction --since 30d`.
- **Nobody is woken.** A report is a record, not a task or a message.
- **The briefing tells agents to use it,** once for each thing, when it happens.

## Why

Friction is how Gathr learns what not to repeat, and it was kept by hand: each agent ended an answer with a
"Friction:" line, and the lead copied those lines into a file. In the team's first three days in Gathr, five of
the owner's seventeen corrections were asking whether friction had been written down. A line at the end of an
answer is lost when nobody copies it; a tool call is kept the moment it is made, with who made it and on what.

## Turned down

- **A field on `finish_task`.** Friction is met in the middle of work, on turns with no task, and by agents who
  never finish one. It needs its own call.
- **Waiting for the scorecard screen** (the plan's first place for it, with the rest of promise 11). The record
  costs one small table and one tool, and every later stage reads it.
- **Naming the promise it breaks in the report.** That is the ledger's job, and an agent in the middle of work
  shouldn't stop to classify.
- **Sending it to the lead as a message or a task.** It would cost a turn each time, for something nobody has to
  act on at once.
