# Gathr

Gathr is an open-source desktop app for working with your own AI agents, from different providers (Claude Code, Codex and more), on your own computer: an assistant for everyday asks, your own specialists, and a team with a lead when the work is a project, with tasks, plans and reviews, and logging built in, so you can see how the agents work and make it better.

## Status

Early, with a working Windows/macOS desktop window and local browser UI. Claude Code and Codex accounts, team chats, tasks, diagnostics and updates are built. What Gathr is for, and what it promises, is in [`docs/charter.md`](docs/charter.md); the decisions behind it are in `docs/decisions/`.

## Build and check

Needs [Rust](https://rustup.rs) (`rust-toolchain.toml` pins the version), Node 24 (`ui/.node-version`) and [cargo-nextest](https://nexte.st) (`cargo install cargo-nextest --locked`). Run `npm ci` in `ui/` first; repeat it when the lockfile changes.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
bash scripts/check-file-size.sh
node scripts/scrub-frames.check.mjs
node scripts/registry.check.mjs
```

In `ui/`:

```sh
npx biome ci
npx tsc --noEmit
npx vitest run
npx vite build
```

UI tests use Edge on Windows and WebKit on macOS (`npx playwright install webkit` once).
For an isolated seeded desktop check, run from the repo root:

```sh
cargo dev --port 47100 --data ./dev-data --seed --window
```

Use a test folder for `--data`; Ctrl+C stops the service and window.
CI runs the checks on Windows and macOS for every pull request. [`AGENTS.md`](AGENTS.md) says how to use and diagnose a running service.

## Contributing

Read [`AGENTS.md`](AGENTS.md): it is the rulebook for people and agents alike.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
