# 0053: A review asks both questions, and a left review gets its fix

Status: accepted (owner, 2026-11-13, with Stage 2's spec, part G). Date: 2026-11-14.

Built: yes. Proof: `crates/gathr/tests/tasks/review_notes.rs`, `crates/gathr/tests/merge/review_questions.rs`, `crates/gathr/tests/archive.rs`, `ui/src/tasks/TaskReport.test.tsx`.

Builds on [0047](0047-a-report-is-a-record.md) and [0049](0049-gathr-lands-pull-requests.md). Serves promise 10 of [the charter](../charter.md#the-promises), part 10.6 of [the truth map](../truth-map.md): a review checks what fails and what wasn't asked.

## The choice

- **R1: a review gets notes.** `finish_task` takes `notes`, one each: a `kind`, a `file` if there is one, and `says`.
  `wrong` is what fails against the ask. `unasked` is what in the change the ask doesn't need. `suggestion` is an
  idea for the lead, never work. A review also takes `nothing_unasked`.
- **R2: a done review answers both questions.** It says `nothing_unasked: true` and has no `wrong` or `unasked`
  notes. A review that found either is reported left. Each refusal says what is missing and carries a reference:
  a kind that isn't one of the three, notes or `nothing_unasked` on a task that isn't a review, a note with no
  `says`, notes with no `commit`, and new `wrong` or `unasked` notes on a review that is done. A plain repeat of
  done still answers `Already recorded`. A refused report writes nothing.
- **R3: notes are rows.** Each note is a row (`review_notes`, migration 0040) at the commit reviewed. A report's
  notes are the rows written with it, and the task shows the newest report's. Recording done again adds rows.
  `gathr.review.notes` is written for every done review and every report with notes.
- **R4: a left review gives its fix.** When the review settles left with nothing open, Gathr gives the owner of
  the first work task on that PR in the chat a task, "Fix the review of #<pr>", from the reviewer, with the notes
  in its why. It is given in passing, as a child of the review. The review settles blocked, its reason names the
  fix and who has it, and its giver hears that as trouble. When the fix is done and held on its merge, it reaches
  the reviewer on the review itself, which reports again.
- **R4: when no fix is given.** It is recorded as `gathr.review.fix` with its outcome: no PR, or no work task
  names it (`no_work_task`); the reviewer gave a work task on that PR in the same turn (`reviewer_gave_one`);
  only `unasked` notes, and an earlier fix already carried `unasked` notes on that PR in this chat
  (`asked_once`); or the give was refused (`refused: <why>`). A given fix is `given`, on the review turn's
  trace. A `wrong` note always gets a fix. The end of a turn never fails because the give was refused.
- **R5: suggestions are heard, not worked.** When a review settles done, its reason is the note and one line per
  suggestion. They start no turn.
- **R6: the brief says what was asked.** A review's brief names the PR and its head as last seen, and the task
  it answers with its why, or says it answers none. It says to name what the ask doesn't need.
- **The stop belongs to the next change.** This one gives the reviewer the question and the builder its fix. The
  merge tool's own rule is as it was.

## Why

A review that only asks whether the change fails lets code the ask never wanted reach `main`, and nobody sees it
until it costs something. Asking the second question in the same turn costs no turn. A note that ends in a task
for the builder means the lead isn't the one who carries it back and forth. One fix for `unasked` notes alone
keeps a dispute over code the builder wants from looping: the second time, the lead hears of it.

## Turned down

- **Unasked notes with no commit.** A note belongs to the code it was written about.
- **A fix every time.** A reviewer and a builder could pass `unasked` notes back and forth for ever.
- **A fix given at the report.** A report is kept until its turn ends (0047), so a fix would be given for a
  report that may be replaced.
- **A setting for how strict a review is.** The rule is the same for every chat.

## Different from the approved detail

1. `review_notes` has a `fix` column. The once-per-PR rule needs it to tell Gathr's fix from the reviewer's, and
   it lets a fix be traced from its notes.
2. The fix is given when the review settles left, not when it is reported.
3. A `wrong` note always gives a fix. Only `unasked` notes on their own are held to once per PR.
4. A refused give doesn't fail the end of a turn.
5. A review recorded again at a newer commit shows its new suggestions, but has no new done line.
6. Landing's sign-off words now name `nothing_unasked: true`, so a reviewer who follows them isn't refused.

## Changes

- [0049](0049-gathr-lands-pull-requests.md): a clean review also has no `wrong` or `unasked` notes.
- [The scenarios](../scenarios.md): 10g is new.
