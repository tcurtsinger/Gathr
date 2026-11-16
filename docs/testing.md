# Testing

How Gathr is tested: which tests to write, where they go, and the loop that writes them. Read it before the first test of a change. AGENTS.md has the short rules; this is the reference.

## Which tests to write

Test behaviour that can go wrong: logic with branches, parsers and protocols, coordination state and its transitions, logins, security, data that could be lost, and every bug that's fixed. Don't test plain glue, getters, constants, what the type system already guarantees, or a layout a screenshot proves. One test per behaviour, not one per function. A test that can't fail is deleted.

## Seams: where tests go

A seam is a public boundary where behaviour can be observed without reaching inside. Tests live at seams, never against internals, so the code behind them can be rewritten and the tests still hold.

Gathr's seams:

| Seam | Test through it | Catches | Misses |
|---|---|---|---|
| The service API (HTTP and WebSocket) | A real `gathrd` on a temp data folder and a free port | Coordination, storage, events, errors with references | Engine quirks, the UI |
| The `Engine` interface | Each driver fed an engine's recorded raw frames (0011, 0019) | Protocol parsing, interrupts, resume, login errors | A provider changing its protocol tomorrow |
| Gathr's MCP tools | An MCP client calling the service, as an agent does | Who may do what, tool results | How an engine presents the tool |
| `gathr diag` | The command against a seeded data folder | That failures are findable from their reference | Nothing it reads is missing from the record |
| Shared UI components | Rendered in the browser, used by keyboard and pointer | Behaviour: states, focus, keys | Looks (screenshots prove those) |

**The seams for a change are named in its plan,** with a line each on what they catch and miss. The owner approves the plan, the reviewer checks the tests sit on those seams. A test at a seam the plan didn't name needs a reason in the PR.

## Mock only at the system's edge

Replace only what Gathr doesn't own, and only with the stand-ins below:

- **Engines:** replayed raw frames, or the stand-in engine. Never a live Claude or Codex login in a test.
- **Decisions:** the local stand-in provider (0014). No key, no network.
- **Time:** an injected clock. No `sleep` and no real waiting: advance the clock, or wait on the event that says it happened.
- **The network** beyond this machine (GitHub, Tailscale): a stand-in server.

**Never mock our own code.** SQLite is real, in a temp folder. The service is real, on a free port. A function that's hard to test without a mock is a design to fix, not a mock to write.

Design so the edge is easy to swap: pass the clock, the engine and the provider in, and give each external operation its own function rather than one generic call with branches.

```rust
// Good: the engine comes in, one operation per method.
fn start_turn(engine: &dyn Engine, session: &Session, input: &str) -> Result<TurnId>

// Bad: builds its own engine from the environment; can only be tested live.
fn start_turn(session: &Session, input: &str) -> Result<TurnId>
```

## Good and bad tests

A good test reads like a line of the spec, uses only the seam, and has one logical assertion.

```rust
// Good: behaviour through the API.
#[test]
fn a_paused_chat_holds_a_new_task_until_resume() {
    let gathr = TestService::start();
    let chat = gathr.chat_with(&["supervisor", "member"]);
    gathr.pause(&chat);
    let task = gathr.give_task(&chat, "member", "write the migration");
    assert_eq!(gathr.task(&task).state, "ready");      // held by the gate, not started
    gathr.resume(&chat);
    assert_eq!(gathr.wait_for_state(&task, "working").state, "working");
}

// Bad: reaches inside. Breaks on a refactor with no change in behaviour.
#[test]
fn pause_sets_the_flag_in_the_scheduler() {
    let scheduler = Scheduler::new();
    scheduler.pause(chat_id);
    assert!(scheduler.paused_scopes.contains(&chat_id));
}
```

**Anti-patterns** (each one is a review note):
- **Coupled to the implementation:** mocks our own modules, tests a private function, asserts on call counts or order, or checks through a side door (reading the database instead of the API). The tell: it breaks on a refactor that changed no behaviour.
- **Tautological:** the expected value is computed the way the code computes it, so the test can't disagree with the code. Expected values come from somewhere independent: a known literal, a worked example, the spec, a recorded frame.

  ```rust
  // Bad: recomputes the answer.
  assert_eq!(total_tokens(&turns), turns.iter().map(|t| t.tokens).sum());
  // Good: a known answer.
  assert_eq!(total_tokens(&[turn(1_200), turn(300)]), 1_500);
  ```
- **A position checked as a boolean:** `expect(atTop(id)).toBe(true)` fails with "expected false to be true", which says nothing about how far off it was. Assert on the measured value (`expect(Math.abs(top - 32)).toBeLessThan(1)`), so a failure prints the number (G177).
- **Horizontal slicing:** all the tests first, then all the code. Write one test, make it pass, then the next.
- **Timing-based:** sleeps, fixed timeouts, "usually done by now". The previous app's room tests failed at random for weeks this way, and every builder reran suites to tell flakes from failures. Wait on events and use the injected clock.
- **Reading a size part-way through a transition:** the UI's tests run in a browser that asks for reduced motion (`ui/vite.config.ts`), so a style that honours it is read where it ends. A tick whose height changes from 2px to 4px by such a transition (`ui/src/chat/map.css`) was read at 2px past the wait on WebKit, at random (ChatMap, macOS CI). A transition that ignores the setting (some registry components' widths) still moves: wait for its end, or don't measure it. A test of motion itself turns it back on for that test (`live-lists.test.tsx`, Chromium only: WebKit can't switch it while it runs).
- **Touching the machine:** a test that changes git settings, the person's logins or anything outside its temp folder. A previous-app test once turned the shared repo bare for eight minutes. Tests run in a temp folder with a clean environment.

**A failing test says why:** the test name, what it expected, what it got, and for a service test the request, the response and the reference to `gathr diag trace`. "fetch failed" with no cause isn't an error message.

**Flaky is broken.** A test that fails at random is fixed or deleted the day it's seen, never rerun until green.

**Gathr's words to agents are held by kept samples** ([0056](decisions/0056-gathr-records-which-words-opened-each-turn.md)). A change to a kind's main form changes its sample in `crates/gathr/tests/words/samples.rs` and raises its version in `crates/gathr/src/words.rs`, with a new row in `KEPT` in `crates/gathr/tests/words.rs`; a new main form comes with its sample.

## The trial

A test that runs real Claude Code and Codex, to tell whether a line of Gathr's words to agents changes what they do ([0066](decisions/0066-role-text-tried-on-real-engines.md)). A replay can't show that. It is for builders, never for the app: `crates/gathr/tests/trial.rs` and `tests/trial/` are `#[ignore]` tests, so `cargo nextest run` and CI skip them (`.config/nextest.toml` runs no ignored test unless asked), and nothing of it ships in Gathr.

**What it does.** Each case runs with a line and without it, alternating block by block on one build and one set of engine copies, on the room's own models and efforts. It gives made-up jobs on a made-up project (`crates/gathr/tests/trial/fixture/`: its [README](../crates/gathr/tests/trial/fixture/project/README.md) and [CONTRIBUTING](../crates/gathr/tests/trial/fixture/project/CONTRIBUTING.md)), counts what each agent did from Gathr's records and the diff, and writes `trial/results.json` in its data folder. The rules for what counts and what decides are in 0066.

**Its normal tests do run in CI** (`tests/trial/tests.rs`: the plan, the cap, the diff, T1's extras, each case's scoring, the decision rule and the file's fields, on made-up rows and a real git repo and `node`). Only the two ignored tests start real engines.

**How a builder runs it.** It spends the person's accounts and shares their usage limits: ask first, run it with the room idle, never start it by accident.
1. Prepare a spare data folder (for example `X:\Gathr\Gathr\<name>\trial\data`, its agents in `data-agents` beside it). A debug build downloads no engine by itself: start the folder's service once with `GATHR_CLAUDE_RELEASES=https://downloads.claude.ai/claude-code-releases` and `GATHR_CODEX_RELEASES=https://registry.npmjs.org/@openai/codex` set, after saying their names, sources and sizes to the person (about 350 MB and 490 MB), and run the trial itself with neither set, so no copy changes while it runs. The person signs in to a Claude Code account and a Codex account there.
2. A first small try (one run of T1 and one of T8 on each engine, the lines only) proves the harness and measures what a run costs. From a PowerShell in the worktree:

   ```powershell
   $env:GATHR_TRIAL_DATA='<the data folder>'
   $env:GATHR_TRIAL_COMMIT=(git rev-parse HEAD)
   $env:GATHR_TRIAL_FIRST='1'
   Remove-Item Env:GATHR_TRIAL_STANDIN -ErrorAction SilentlyContinue
   cargo nextest run -p gathr --test trial --run-ignored only --no-capture -E 'test(=trial)'
   ```

   It prints the runs and the most they can cost before any turn, then writes `<data>\trial\results.json` and `log.txt`. It must be a debug build, as here: a release build ignores the switch, so both arms would be the same, and the trial refuses to start. `GATHR_TEST_KEEP` must be unset: on a failure the harness would copy the whole signed-in folder there, logins included. The trial refuses it, and refuses the person's own Gathr folder.
3. The full trial is the same variables without `GATHR_TRIAL_FIRST` (`Remove-Item Env:GATHR_TRIAL_FIRST`), in the background with its output sent to a file. Never set `GATHR_TRIAL_STANDIN`, and never set `GATHR_TRIAL_WITHOUT` by hand: the harness sets it for each block, and refuses to start with it set. No block starts past 90M input tokens. Each run's records must show its arm, or the trial stops.
   To end a run early, stop the `cargo nextest` run and its `gathrd` (the one started with `--data <the data folder>`) by their process ids, never by name. The next start of the trial archives the chats the stopped one left, so their queued turns never start unwatched. Before each block, the project keeps only its `main` and `trial/planted` branches.
4. To check the harness without any real engine, set `GATHR_TRIAL_STANDIN=1` and a throwaway data folder: it runs one block of each case, both arms, on the replay stand-in.
5. Afterwards copy `<data>\trial\results.json` to `crates/gathr/trial/results.json`, delete each dropped line with its constant, its `SHIPPED` entry and its role samples, record each line's decision in 0066, and delete the data folder and `data-agents`. Stop what you started, by its process id.

**The gate.** `crates/gathr/trial/results.json` is committed, and `tests/roles.rs::the_shipped_lines_passed_the_trial` fails unless every line in `crates/gathr/src/roles.rs` has, in that file, its exact text's SHA-256 with the decision `kept`. A change to a line's words therefore needs a new trial, and a contributor with no Claude Code and Codex accounts can't change one. The gate checks the text, not what surrounds it: 0066 lists what calls for a new trial although the gate stays green. The file holds a closed list of fields (no path, name, id or free text), checked by `the_trial_file_holds_only_its_fields`.

## The loop

1. Take the plan's next seam.
2. Write one test there.
3. Run it: it fails, for the reason you expect (red).
4. Write the least code that passes it.
5. Run that test again: it passes (green). Only this one; earlier tests wait for the run before pushing.
6. Next behaviour: back to 2.

Rules:
- **Red before green.** Watch it fail first; a test you've never seen fail may not test anything.
- **One slice at a time.** One behaviour, one test, one minimal change. Nothing speculative.
- **Refactor once the behaviours are green,** as one pass at the end: structure only, then one run of the tests it touched.
- **A bug starts with its test.** Reproduce it at a seam (for an engine bug, `gathr diag turn <id> --raw` saves the frames as a fixture), watch it fail, then fix it.
- **Run the tests the change touches once before pushing,** not the whole suite and not after every step. CI runs everything.
