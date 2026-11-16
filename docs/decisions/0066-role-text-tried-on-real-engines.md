# 0066: Role text is two lines, each shipped only after a trial on real engines

Status: accepted (owner, 2026-11-16, with Stage 3's spec, part K; the owner's answers of 2026-11-16: "Just us. This is just to make Gathr better.", "We dont touch effort. That is completely configurable by the user in the agent settings." and "The room's real work."; its detail and its departures, "Approve K"). Date: 2026-11-16.

Built: yes. Proof: `crates/gathr/tests/roles.rs`, `crates/gathr/tests/trial.rs`, `crates/gathr/trial/results.json`.

Builds on [0021](0021-no-prompting-needed.md), [0053](0053-a-review-asks-both-questions.md), [0056](0056-gathr-records-which-words-opened-each-turn.md) and [0057](0057-agreeing-is-a-reaction.md). Amends 0021, 0053 and 0057, below. Serves promise 11 of [the charter](../charter.md#the-promises), part 11.1 of [the truth map](../truth-map.md), and parts 8.7 and 6.6.

A **line** is one sentence of role text, or of a brief, that Gathr adds. A **case** is one fixed test of what an agent does (T1 and T8 now; T2 and T5 were tried and removed). A **case run** is one case, once. An **arm** is one of the two ways a case runs: **with** the lines, or **without** them, as Gathr was. The **trial** is all the case runs, both arms, in one sitting. In examples Ana is the lead, Bo and Cy are members.

## The choice

- **K1. Two lines ship, each kept by the case that measured it.** Four were tried. Each says only what no tool or state in Gathr already makes an agent do.

  | Line | Where it goes | Measured by | Decision |
  |---|---|---|---|
  | L1, the lead: `Before you ask the person, look in the code and docs: ask only what they don't answer, with choices and your pick first.` | The briefing, for a Supervisor in a team | T5 | dropped, removed |
  | L2, a member: `Make the smallest change that does what was asked: no fallback, guard, abstraction or compatibility nobody asked for, and remove what it replaces.` | The briefing, for a member in a team | T1 | kept |
  | L3, everyone in a team: `When you only agree with another agent's message, give it a thumbs-up with react instead of replying.` | The briefing, every role, in a team | T8 | kept |
  | L4, a code review: `Say what is wrong or missing against the ask, as wrong notes.` | The review's brief, before its unasked line, as `review_brief` version 2 | T2 | dropped, removed |

  L2 and L3 are in `crates/gathr/src/roles.rs` (`lines(role)` for the briefing, `SHIPPED` for the gate), after the briefing's first line and only in a chat of two or more agents, where the role is already named. A member gets L2 and L3; a lead and a reviewer get L3 alone. L1 and L4 were deleted with their constants, their `SHIPPED` entries, their samples and their cases. Commit `ba2921c` holds them as tried.
- **K2. The lead and the reviewer have no line of their own.** The reviewer's half on plans is in the plan check's brief, and "with file and line" is K4. L1 and L4 made no difference, so the briefing and the review's brief are as before them. A member who reviews reads the member's line: roles are a chat's, not a task's.
- **K3. The without arm is one switch, for debug builds only.** `GATHR_TRIAL_WITHOUT`, read in `roles::without` and only when `cfg!(debug_assertions)`: set, `roles::lines` gives none. A release build never reads it. Both arms then run on one build, the same engine copies and the same day. No chat or agent carries a marker a model could see. While L4 was in the code, the switch also gave the review brief's words without it.
- **K4. A `wrong` note in a code review names its file.** On a review that isn't a plan check, `finish_task` refuses a `wrong` note with no `file` (or a blank one), nothing written: `Name the file of each wrong note, with its line: path:line. Something missing is in the file where it belongs.` A plan check's notes are unchanged: a plan's wrong fact or wrong owner has no file. Gathr enforces it, so no line and no measuring.
- **K5. Each turn's start records the role and its version.** `gathr.role` when the briefing names a role, and `gathr.role.version` (`words::ROLE`, 1) when that role got lines, on the `invoke_agent` start event. `words::ROLE` is a kind with samples (`role: supervisor`, `role: member`, `role: reviewer`) and a kept row. `REVIEW_BRIEF` was 2 while L4 was in the code and is 1 again, as on `main`: no build with 2 was released.
- **K6. The trial is a test in the repository, and only a builder runs it.** `crates/gathr/tests/trial.rs` and `tests/trial/` are ignored tests (`real engines: Claude runs it by hand`) that neither `cargo nextest run` nor CI runs. Nothing of it is in the app: no Settings page, command or route. It runs on a spare data folder with its own sign-ins, on the room's own models and efforts, and Gathr never picks an effort. [docs/testing.md](../testing.md#the-trial) says how it runs.
- **K7. The cases.** Each runs on the engines the room uses for that role:

  | Case | Engine | What is given | What is counted |
  |---|---|---|---|
  | T1 Build (L2) | Claude Code and Codex | A member, a task from its paused lead: a small feature (`formatDuration` and a `--format` option) with room to over-build | Whether the run is good: its hidden test passes and it isn't over-built. Over-built is any extra (a new `src/` file, a name exported beyond `formatDuration`, an option beyond `--format`, a parameter beyond `ms`, a line in `src/duration.js` guarding input the ask never mentions) or more than 70 lines added in `src/`. A done report is counted too |
  | T8 Agree (L3) | Claude Code and Codex | A member; a message from its paused lead that needs only agreement and leaves nothing to do | A thumbs-up and no `message` call. The turn's closing words are counted, not judged: they reach the chat and start no turn of the sender's |
  | T2 Review (L4), removed | Codex | A reviewer, a review of a commit with a planted fault, its test for the upper-case ask, and an unasked `src/retry.js` | A `wrong` note on `src/duration.js`, an `unasked` note naming `retry.js`, and no `wrong` note elsewhere |
  | T5 Ask (L1), removed | Claude Code | A lead with a member in its chat; a message with two open choices, camelCase or snake_case keys (answered in the project's `CONTRIBUTING.md`) and whether the object carries the text it read | Questions asked; whether one asks the answered choice (its words and choices name two key styles, or say "key style" or "key case"); whether one has choices |

  T2 and T5 were tried and then removed with their code: with their lines gone, both arms of each would run the same. The project is made up and lives with the test (`crates/gathr/tests/trial/fixture/`, about 200 lines, CommonJS): its [README](../../crates/gathr/tests/trial/fixture/project/README.md) and [CONTRIBUTING](../../crates/gathr/tests/trial/fixture/project/CONTRIBUTING.md) are what the agents read. Its planted branch and the `CONTRIBUTING.md` line on JSON keys went with T2 and T5. No model judges another: every count is read from Gathr's records and the diff.
- **K8. What decides whether a line ships** (`tests/trial/decide.rs`). Each rule counts runs, never a median. A case on two engines is decided on both together and must not be worse on either. A run that is `spoiled`, `usage_limit` or `signed_out` doesn't count; an arm with fewer than two thirds of its runs makes the line `undecided`, and it doesn't ship; so does a trial whose resolved models changed between its start and its end.
  - **L2** (T1, 6 runs an arm on each engine): at least 4 more good runs of 12, and fewer on neither engine. A run that timed out, went over its cap or changed nothing is not good, and still counts as a run.
  - **L3** (T8, 6 runs an arm on each engine): at least 4 more passing runs of 12, and fewer on neither engine.
  - As tried, and removed with their cases: **L1** (T5, 8 runs an arm) needed at least 3 fewer runs that ask the answered choice, or at least 3 more whose questions have choices, worse on neither, and at most one fewer run asking anything; a T5 run that timed out or went over its cap didn't count. **L4** (T2, 6 runs an arm) needed at least 3 more passing runs.
- **K9. How the trial runs.** A list of blocks, each one arm and one run of every case, arms alternating block by block and the first arm alternating by round: six rounds, twelve blocks. Tasks are written with the service stopped, given by a paused lead (Ana), so the brief reads as real use's does. Each case run has limits (T1 4M input tokens and 30 minutes; T8 1M and 5). An approval an engine asks for is answered no and the run is `spoiled`. Each run is scored before its chat is archived. No block starts once the trial has spent 90M input tokens. A usage limit or a sign-out ends the trial after its block, keeping what ran. The detail expected about 80M input tokens and 3 to 4 hours. Measured: the first small try (one T1 and one T8 on each engine) 0.5M and 58 seconds; the full trial with all four cases, 76 runs in 16 blocks, 10.25M and 21 minutes, of which its T1 and T8 runs spent 6.4M.
- **K10. The gate.** `crates/gathr/trial/results.json` is committed and holds the trial's result. `tests/roles.rs::the_shipped_lines_passed_the_trial` fails unless every line in `SHIPPED` has, in that file, its exact text's SHA-256 with the decision `kept`. The file is read at run time, so the first commit compiled with the placeholder `{"format": 1, "lines": []}` and the gate failed until the trial's file replaced it. On the committed file it passes: L2 and L3 are `kept`, and L1 and L4, `dropped`, are no longer shipped.
- **K11. The file holds a closed list of fields.** It is public. Only these fields, each a number, a boolean, a hash, a date, a version, a model name or one of a listed set of words: `format`, `date`, `gathr` (the build's commit), `copies`, `words`, `lines` (`id`, `sha256`, `case`, `decision`), `runs` (the counts of each case), `spent` and `ended`. No path, account name or id, chat or task id, email, machine name or free text, and an engine's error message never goes in. `tests/roles.rs::the_trial_file_holds_only_its_fields` checks it, the committed file included, and the trial checks its own output with the same list (`tests/roles/results.rs`). The list still names T2, T5, L1 and L4 and their counts: the committed file holds their runs and rows.
- **K12. Diagnosable.** `gathr.role` and `gathr.role.version` on each turn's start; K4's refusal as `gathr.tool.call` with its why; the trial's own record is its data folder, read with `gathr diag --data <folder>`. The forced failure: a `wrong` note with no file, found from its reference with `gathr diag trace` (`tests/roles.rs::a_code_reviews_wrong_note_with_no_file_is_refused_and_found_from_its_reference`).

## The trial's result

The trial ran on 2026-11-16 at commit `ba2921c`, on Claude Code 2.1.325 and Codex 0.169.0, at the room's settings: `opus` at high effort on Claude Code (`claude-opus-5-5`), `gpt-6.1-sol` at medium on Codex, and the reviewer's `latest:sol` at high (`gpt-6.1-sol`). 76 runs in 16 blocks, 21 minutes, 10,250,872 input tokens with cache. No run was spoiled, timed out, went over its cap or asked an approval, and the models were the same at the end. Its file is `crates/gathr/trial/results.json`.

| Line | Case | Decision | Tried on | With the line | Without it |
|---|---|---|---|---|---|
| L1 | T5 | dropped | Claude Code | 8 of 8 runs asked one question, with choices, about the unanswered choice only | The same, 8 of 8 |
| L2 | T1 | kept | Claude Code and Codex | 12 of 12 good (Claude Code 6, Codex 6) | 1 of 12 (Claude Code 0, Codex 1) |
| L3 | T8 | kept | Claude Code and Codex | 12 of 12 passed | 3 of 12 (Claude Code 3 of 6, Codex 0 of 6) |
| L4 | T2 | dropped | Codex | 6 of 6 passed | 6 of 6 |

- **L2.** Every over-built run had one guard line in `src/duration.js` (an input check that throws), and added more: about 24 lines in `src/` with the line and 38 without on Claude Code, 18 and 28 on Codex. Read by hand: without the line Claude Code also added a test for bad input, and once a README section it said nobody asked for. Claude Code's T1 runs averaged 29 seconds and 220K input tokens with the line, 103 seconds and 306K without; Codex's were the same either way (57 seconds, 139K).
- **L3.** Without the line Codex sent the lead a `message` every time.
- **L1.** Read by hand: every lead, in both arms, found the key style in `CONTRIBUTING.md` and said so in its reason.
- **L4.** Every review, in both arms, put a `wrong` note on `src/duration.js`, named `retry.js` as unasked, and found nothing wrong elsewhere.
- Before it, the first small try (four runs, 506,259 tokens, 58 seconds) led to the two changes under "As built".

## Why

- Every sentence in a briefing costs tokens on every turn, so one is worth keeping only if it changes what Claude and Codex do. A replay can't show what a model does with new words, so a line is tried on real engines.
- Each case runs with and without a line, alternating on one build in one sitting, because a line that was never compared with its absence proves nothing about the line.
- Four lines were tried, not about fourteen: each other line the spec had is already made to happen by a tool or by state (sizing, a request in the lead's words, complete plan parts, look-into before building, answering every finding). Two shipped: the other two changed nothing, and would have cost tokens on every turn.
- K4 is a refusal, not a sentence: a tool can check that a note names a file, so no model has to be asked to.
- The trial is outside the app because it serves the people who build Gathr, not its users, and it spends their accounts.

## What was turned down

- **The spec's Settings Run button, `gathr trial` command, routes, events and a project inside the service** (the owner: "Just us").
- **Default efforts, an effort sweep or any wording about effort** (the owner: "We dont touch effort").
- **A team case that measures a team against one agent** (the owner: "The room's real work"). Whether a team beats one agent is read from the room's own record once it runs on this build: tokens by building, checking and coordination, and how often a check changed the work. What one agent alone would have spent isn't recorded and can't be read.
- **Showing the lines in Settings.** They are in this decision, in `roles.rs` and, with the date they were tried, in `results.json`.
- **A line for something a tool or state already enforces,** and a reviewer role line.
- **"Ask the teammate who knows"** as a line: no case measures it, and AGENTS.md keeps its rule.
- **Two builds for the two arms:** they would double the build, the engine copies and the sign-ins, and add drift.
- **Models grading models.** Every count is read from the records and the diff.
- **A median of a case's numbers:** one run moves a median of six by tens of lines.
- **Keeping L1 and L4 because they did no harm,** or keeping T2 and T5 in the harness: a line costs tokens on every turn it's in, and a case without its line runs both arms the same.

## What it amends

- **0021**, "how it works" per role: "comes with a default "how it works" the person can edit" becomes "comes with a few built-in lines, each shipped only after the trial showed it changed what agents do (0066)". "No assumptions, without being told" in 0021 stays as it is.
- **0053**: a `wrong` note in a code review names its file, as `path:line`. Only that rule: the review's brief is as 0053 left it.
- **0057**: L3 is how the briefing tells agents to agree with a reaction.

## What calls for a new trial

The gate checks a line's text, not what surrounds it. A new trial is called for, although the gate stays green, by: a change to the briefing around the lines, to the description of a tool a line names (`react`), to a brief a case opens with, or a new engine family. A new line needs its case added back to the harness ([docs/testing.md](../testing.md#the-trial)). Nothing enforces this list.

## Said plainly

- **Half the lines didn't ship.** Leads already looked in the project's docs before asking, and Codex reviewers already named what was wrong, so L1 and L4 changed nothing. That is the point.
- **The rules are a screen, not a proof.** A line that does nothing still passes by chance a little under 1 time in 10 at a 50% rate. A line passes only with a large effect, about a third of the runs; one that adds good runs by 10 or 20 points will usually fail, and a line left out may still help a little. Lines were tried together, each read off its own case: whether one changes another's case isn't measured.
- **L3 was tried on members only.** A lead or a reviewer reads it untried.
- **Removing L1 changed the lead's briefing around L3.** L3 now follows the lead's first line, where L1 was. The trial didn't run a lead's briefing in that form.
- **The trial's project is tiny.** A clear effect there is a reason to ship a line, not proof it works on Gathr's own code. The room's record after release is.
- **A contributor with no Claude Code and Codex accounts can't change a line:** the gate fails, and only a builder with accounts can run the trial.
- **The window doesn't show the lines.**
- **A trial shares the person's usage limits** and must not run while the room works. A second sign-in to the same account in another folder is its own login (0002); that the provider never ends an older login when a newer one is made on the same account is not verified.

## As built

What differs from the detail, from the code as it landed:

- **`gathr.role` is recorded whenever the briefing names a role; `gathr.role.version` only when that role got lines.** The trial tells the arms apart by `gathr.role.version`, which is null on the without arm's starts (`crates/gathr/src/bin/gathrd/turns/process.rs`, `opening.rs`).
- **K4 also refuses a whitespace-only `file`** (`crates/gathr/src/tasks/report.rs`).
- **`SHIPPED`, the list of shipped lines as `(id, text)`, is in `roles.rs`.** Each line is one constant plus one entry, so L1 and L4 were removed each on its own.
- **The gate's and the shape check's functions** (`line_sha256`, `unkept`, `shape`) are test code in `tests/roles/results.rs`, included with `#[path]` so the trial harness uses the same file. No dependency was added.
- **The block's writes ran in a child process of the trial's own test binary** while L4 was in the code, because the review's brief read the switch in the process that gave the tasks, and the workspace forbids `unsafe`, which rules out `std::env::set_var`. With L4 gone no write reads the switch, and the trial writes each block in its own process (`tests/trial/setup.rs`).
- **Chats are made through the library in the stopped-service write,** not by `POST /api/chats` and `PATCH`: the calls those routes make, and a service start saved per block. Every trial chat is titled "Durations", because the briefing quotes the title and it must not name the case or the arm.
- **T8's message is shown as Ana's,** in a turn of hers saved as already done, and Gathr's copy is queued as a teammate's message with the shared format (`gathr::handoffs::told`, used by the `message` tool). The hand-off counter isn't recorded for it.
- **T1 runs that reach their end get the outcome `compared`,** because T1 is decided by counts of good runs. T8 used `passed` and `failed`, as T2 did; T5 also had `no_question`.
- **After an approval is answered no, the run's chat is also stopped,** to save tokens.
- **The build's commit comes from `GATHR_TRIAL_COMMIT`,** required except on the stand-in: the harness runs no `git rev-parse` on this repository.
- **The file is written to `<GATHR_TRIAL_DATA>\trial\results.json`,** rewritten after every block and pretty-printed at the end, never to `crates/gathr/trial/results.json`; the reviewer copies it in. A log is kept beside it in `trial\log.txt`.
- **The hidden test runs as `node format.hidden.test.mjs`** from the worktree root, not `node --test`, so there is one process to time out and kill. Before diffing, `git add --intent-to-add .` runs in the agent's worktree so new files show.
- **On the stand-in, case time limits are capped at 20 seconds,** and blocks wait one shared 60 seconds for stopped chats' turns to end before scoring them as they stand.
- **The review's fixes before any real run.** Each departs from the approved detail, so that the trial doesn't spend the person's tokens on a wrong or biased measurement:
  - **T1 counts good runs, not over-built ones** (K8's L2). Under the detail's rule a run that built nothing, timed out or went over its cap counted as "not over-built", and its hidden-test guard let one such run in on each engine, so fewer over-built runs could come from runs that built nothing that works.
  - **T1's guard words count only in `src/duration.js`.** The project's own `src/cli.js` already checks `Number.isNaN` and its rules say how errors are reported, so following them there was counted as over-building.
  - **T5 and T2, as tried:** T5 left out runs that timed out or went over their cap, named camelCase and snake_case in its message, and counted a question as asking the answered choice only when its words and choices named two key styles, or said "key style" or "key case". T2's planted change had a test for the upper-case ask, so a correct note about a missing test couldn't fail the reviewer, and a note's path with a worktree's folder before `src/` counted as that `src/` file. These went with T2 and T5.
  - **Each run's records must show its arm, or the trial stops** (`check_arm` in `tests/trial/block.rs`): every turn start of the run's agent has `gathr.role.version` with the lines and none without. Until T2 went, it also checked that T2's brief carried L4 only with them. The trial also refuses to start with `GATHR_TRIAL_WITHOUT` set, or in a release build, which ignores the switch.
  - **A killed trial's chats are archived at the next start** (`setup::clean`), with the service stopped, through `gathr::archive::archive`, the call the archive route makes: it stops queued and sign-in-waiting turns and cancels the chat's tasks. A turn left `running` is marked failed by the service at its start and never resumed.
  - **Before each block, the project's repository keeps only `main`** (`project::forget`). Archiving removes a chat's worktrees but keeps their branches, and keeps a worktree with changes not committed, so an agent could read an earlier run's work with `git log --all`.
  - **The trial refuses the person's own Gathr folder and a set `GATHR_TEST_KEEP`** (a failing test would copy the signed-in folder there), and the `results.json` written after each block decides nothing: every line is `undecided` until the end.
  - **Probes that only logged were deleted:** the stand-in's briefing read-back, Ana's held turns before and after archiving, T1's work turn and worktree, and T2's brief line. The stand-in run settled them, and `check_arm` asserts the part that matters.
- **Size,** as `git diff --numstat origin/main` counts added lines. Shipped code 64 (28 in `roles.rs`, 36 elsewhere) against the detail's 80. The trial's harness 1,461 lines against about 860, as in parts F and J: rustfmt, the stand-in engines, the arm check and the start's cleanup; with T2, T5 and the self-exec writer it was 1,707 at `ba2921c`. The fixture 202 lines. Tests CI runs: 539 in `tests/roles.rs` and `tests/roles/results.rs`, 35 in other tests and 548 in `tests/trial/tests.rs`, against about 180, 50 and 280. The trial's file 1,745 lines. No migration.
- **Existing tests changed:** wrong notes in tests that aren't about K4 gain a `file`, because K4 now refuses them without one. Plan checks' tests are unchanged on purpose. The review brief's expected line gained L4 for the trial and is as on `main` again.
- **What the first small try on real engines changed** (2026-11-16, the lines only; the owner: "Both engines", "Yes, change it"):
  - **It runs one T1 and one T8 on each engine,** not on Claude Code alone: more than half of the trial is Codex, and whether Codex runs the project's tests without asking was worth knowing first. It did.
  - **T8 passes with a thumbs-up and no `message` call.** The detail also asked for no reply text. Claude Code gave the thumbs-up, sent nothing, and closed its turn with a sentence, as engines do. Those words reach the chat and started no turn of the sender's, so they are counted and not judged.
  - **T8's message is "Plan for tomorrow: you take the parser, I review after. Nothing to do yet. OK?"** The detail's "Plan: you take the parser, I review after. OK?" read as work to start: Codex looked through the project and sent the lead a `message` asking for requirements.
  - **Measured:** T1 214K input tokens and 35 seconds on Claude Code, 125K and 55 seconds on Codex, both small and working; T8 83K and 84K. No approval was asked.
- **Checked on the replay stand-in,** end to end: one block of each case, both arms, and again after a trial was left aborted (the next start archived its chat and stopped its queued turn). The arm check was seen to stop the trial with its sense reversed. After T2 and T5 were removed, one round of T1 and T8 ran again on the stand-in, both arms, each run's arm checked. On real engines, the first small try and the full trial ran.
- **Not verified:** that a release build never reads `GATHR_TRIAL_WITHOUT` (guarded by `cfg!(debug_assertions)`, which no test can see); the paths no run took (`over_cap`, the 90M cap, `usage_limit` and `signed_out` ending the trial); a real engine's stopped turn, how promptly it ends and whether its usage is counted; that the hidden-test check finds `node` in CI's Rust job; macOS.
