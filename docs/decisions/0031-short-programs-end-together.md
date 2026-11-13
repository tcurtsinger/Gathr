# 0031. Short programs end with everything they started

Status: accepted (`process-wrap` approved by the owner, 2026-11-09). Date: 2026-11-09. Amends AGENTS.md's "one way
to spawn a process".

Built: yes. Proof: `crates/gathr/tests/run.rs`.

## Choice

The programs the service runs and reads (`gathr::run`: git, gh, a wait's check) start through
[`process-wrap`](https://crates.io/crates/process-wrap) 10.0.1 (Apache-2.0 OR MIT), with only the features they use
(`tokio1`, `job-object`, `creation-flags`, `process-group`):

- **Windows:** a Job Object per program, with no console window. Windows puts each process the program starts into the
  job as it's created, so ending the job ends them all, including a child Git Bash was starting when it was ended,
  which neither MSYS nor any process tree lists yet.
- **macOS:** its own process group, ended as one. The group's id stays the program's until it's waited for, and it's
  waited for only once its output ends.

Engines keep how they start and their gentle close: they may be renewing a login (0002), and a job or group kill ends
processes outright.

## Why

Git Bash ended while it forks leaves the fork's child waiting for it for 300 s (MSYS2's `FORK_WAIT_TIMEOUT`), holding
the program's output open. #63 stopped that from holding up the service's exit (its output is read on threads of its
own) and ended the MSYS process group, but the group kill can't see a child MSYS hasn't registered yet: 1 to 4 in 20
were left behind, and `cargo test` waited on them. With the job, a test that ends 40 checks mid-fork leaves none (it
left 8 before).

It also replaces about 40 lines of our own group kill (an MSYS `/proc` walk through Git Bash on Windows, `kill` on a
group on Unix).

## Turned down

- **`windows-sys` and about 70 lines of `unsafe` code** for the job: the repo forbids `unsafe`, and `process-wrap`
  already handles the gap between a child starting and its joining the job (it starts the program suspended and
  resumes it once assigned).
- **`win32job`:** small, but no release since May 2025.
- **`taskkill /T`** (as SerenityOS does): it follows Windows' parent links, which a child stranded mid-fork doesn't have.
- **Every process Gathr starts through `process-wrap`:** engines must never be ended outright.

## Cost

Two crates new to the build: `nix` 0.31.3 (MIT, macOS only, through `process-wrap`'s `tokio1`) and its build helper
`cfg_aliases` 0.2.2 (MIT). `windows` 0.62.2 was already in it; the service now names it directly for one type.
