# 0056: Gathr records which words opened each turn

Status: accepted (owner, 2026-11-14: Stage 3's part A, then "Kept samples" and "Main forms"). Date: 2026-11-14.

Built: yes. Proof: `crates/gathr/tests/words.rs`, `crates/gathr/tests/tasks/words.rs`, `crates/gathr/tests/quiet/words.rs`, `crates/gathr/tests/stale.rs`, `crates/gathr/tests/merge/lands.rs`, `crates/gathr/tests/app_update.rs`, `crates/gathr/tests/pauses.rs`.

Builds on [0011](0011-event-record.md) and [0019](0019-troubleshooting.md). Serves promise 11 of [the charter](../charter.md#the-promises), part 11.5 of [the truth map](../truth-map.md): a change to Gathr's words to agents is measured, not guessed.

## The choice

- **A1: each turn records its kind.** Two columns on `turns` (migration `0043_turn_opened.sql`): `opened_by` (the kind of Gathr's text that opened the turn) and `opened_version` (that kind's version when the turn was made). Both are NULL on turns made before the migration.
  - `store::add_turn`, the service's one insert of a turn, takes the kind as a `gathr::words::Opened`.
  - The kinds are `brief`, `review_brief`, `request_brief`, `report_ask`, `hear_back`, `answered`, `wake`, `silent`, `wait_fired`, `stale`, `landing`, `carry_on`, `person`, `teammate`, `compaction` and `engine`. `Opened::ALL` lists them.
  - The first twelve have fixed text, so a version. `person`, `teammate`, `compaction` and `engine` don't: a message is not Gathr's words, `/compact` is the engine's command, and a turn the engine began by itself has no ask. Their version is NULL.
  - The kind is `landing`, not hand-off: in this code a hand-off already means a message between agents.
  - A message the engine reads only after its turn keeps its sender's kind, `person` or `teammate`, whether it gets its own turn or is queued again.
  - The kind is set once, at the insert. A teammate's message joined to a queued turn doesn't change that turn's kind.
- **A2: a kind's version is held by kept samples of its text.** Gathr's words stay where they are built. No word moves or changes in this decision.
  - Each kind with fixed text has a version constant in `crates/gathr/src/words.rs` (`BRIEF`, `REVIEW_BRIEF`, and so on), all at 1.
  - `crates/gathr/tests/words/samples.rs` holds 24 samples: one text of one kind, as the real code builds it from a fixed made-up input, kept as a literal. Each test that builds a sample calls `samples::same(name, built)`, which fails when the built text differs from the kept one, and also asserts the turn's kind and version.
  - `crates/gathr/tests/words.rs` holds `KEPT`: one row of (kind, version, SHA-256 of the kind's samples) for each version. Rows are only ever added; an edited or removed row is a review finding.
  - The rule is a function, `check`, over the kinds, the samples, `KEPT` and the source of the test files that build samples. It fails with a message that says what to do for each of these: the samples' hash isn't the one kept for the version; the version has no kept row; the version is below the highest kept; a kind with fixed text has no sample; a kind with no version has samples or kept rows; a sample no test asserts; two samples with one name.
  - Adding or editing a sample changes the hash, so the kind takes a new version even with the same words. That costs less than a branch left unguarded.
- **A3: `gathr diag words`.** `gathr diag words [--since 7d]` counts, for each kind and version over the turns made since then:
  - `turns`: every turn.
  - `no_tool`: turns that ended `done` with no tool message.
  - `finish_refused`: `finish_task` calls turned away on the turn, from the `gathr.tool.call` events on its trace.
  - `no_report`: task turns that ended `done` with no report and nothing open, from the task's own `gathr.task.state` event on the turn's trace: the one ask for a report (`ready`), or the reason `NO_REPORT`. A task held on a wait or a command owes no report, so it isn't counted.
  - A turn with NULL columns counts under the kind `unrecorded`.
  - The count is one library function, `gathr::words::counted`, so part K's trial reads the same numbers.
  - On a database below schema 43, `diag words` runs no query and says `Nothing is recorded before schema 43. This database is at schema <v>.`, and `diag turn` leaves the two fields out. `gathr diag` reads any database at or below this build's schema.
- **A4: the rule for later changes.** A change to a kind's main form changes its sample, raises its constant by one and adds its kept row. A new main form for a kind adds its sample. Review checks both. A part that adds a kind adds it to `Opened`, with its constant at 1 and its samples.
- **Samples, by kind** (the main forms only):

  | Kind | Samples |
  |---|---|
  | `brief` | no why; with a why; picked up again with "Continue from here" (one carry reason, the restart's) |
  | `review_brief` | a PR whose head is seen, answering a task; a review with no PR |
  | `request_brief` | a choice with the member's pick; free text |
  | `report_ask` | the one ask |
  | `hear_back` | one task settled done; several settled; one that stands |
  | `answered` | an answer to the agent's own question; an answer to a member's request |
  | `wake` | one task, at once |
  | `silent` | on a task |
  | `wait_fired` | on a task; on no task; expired |
  | `stale` | one task, one day; several tasks, several days |
  | `landing` | a PR Gathr can't land |
  | `carry_on` | an update; Stop now; a usage limit |

- **What no sample covers.** A change to one of these passes every test and moves no version:
  - the brief's other carry reasons (a failed task, Gathr stopped, the engine closed, a blocker done, a hold ended with or without a note, a merge with the task left, blocked or with no report);
  - the review brief's other forms (a head not seen, a task with no why, no task);
  - the request brief's other answer forms (yes or no, choices, done);
  - the hear-back's other headings (left, no report, several stand);
  - the person's answer when cancelled, and when no member is named;
  - the wake's and the silent turn's second forms (several tasks and an age; no task);
  - the wait's other endings (all of several, an agent that ended well, failed or stopped, an expired wait that failed its last check, an expired PR wait);
  - the stale line for the person's tasks only;
  - the two carry-on forms only `carry::save_all` builds (a turn and commands; commands only).

  Two branches were read and not sampled for a different reason: no public path was found that settles a task with no reason, which the hear-back and the person's answer would show. Not verified that none exists. The waiting answer's other lines and the age's days form are shared with the `waiting` tool. A branch that is added later with no sample is not caught by a machine; only review sees it.
- **Input, not words.** The made-up input supplies names, a task's what and why, a report's note and a reason. A value no input can fix is replaced by a token before the sample is compared: `<head>` (a PR's head as git made it) and `<version>` (the build's).

## Why

- Promise 11: what Gathr says to agents is changed on evidence. Before this, no turn said which of Gathr's words opened it, so a change to the words could not be told from a change in the work.
- A turn's row is the one place that holds it for every turn, whichever path made it. An event attribute on `gathr.chat.give` would show the kind for some turns only: not every turn goes through it.
- A kept sample, not a promise, ties a version to the words: a text that changes without a new version fails a test.
- The reading is built in: `no_report` and `finish_refused` come from what Gathr already records, not from anything an agent is asked to do.

## Turned down

- **Moving Gathr's words into one module** (about fifty texts out of twelve files, filled by macros). It is about twice the size of this part and still needs golden tests on top. It would also sit in the way of every later part that changes words. The owner chose kept samples instead.
- **Pinning every branch of every text.** 53 samples, about 1,100 lines of tests. The owner chose the main forms: 24 samples. What that leaves unguarded is the list above. A branch outside the main forms can change with no new version, so a turn's counts may mix two wordings under one version. A reviewer is the only guard.
- **Hashing the words' fragments** instead of samples: it needs the words moved, and misses how they are joined.
- **A CHECK on `opened_by`:** a new kind would rebuild `turns`.
- **Guessing old turns' kinds from their messages.** Old turns stay `unrecorded`.
- **A `words` table in the database:** the version is the code's.
- **A kind for the session opening** ([0055](0055-a-members-task-starts-in-a-new-session.md)): it isn't a turn's ask.
- **"No report" read as "no `finish_task` in the turn":** a task held on a wait or a command owes no report then.
- **The kind on `gathr.chat.give`:** only some turns pass through it.
- **A screen.** The numbers are read with `gathr diag words`. The scorecard is part 11.3.

## Changes

- [The truth map](../truth-map.md): part 11.5 is new.
- [The scenarios](../scenarios.md): 11e is new.
- [Testing](../testing.md): the samples rule.
- [AGENTS.md](../../AGENTS.md): `diag words` is in the `gathr diag` list.
- [Architecture](../architecture.md): `gathr diag` supports `words`.
