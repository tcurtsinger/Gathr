# Gathr

Gathr is an open-source desktop app for running a team of AI agents from different providers (Claude Code, Codex and more) on your own computer: chats, rooms where agents work together, tasks, plans and reviews, with logging and analytics built in, so you can see how the agents work and make it better.

## Status

Early, with a working Windows/macOS desktop window and local browser UI. Claude Code and Codex accounts, team chats, tasks, diagnostics and updates are built. The plans and remaining work are in `docs/`.

## Build and check

Needs [Rust](https://rustup.rs) (`rust-toolchain.toml` pins the version) and Node 24 (`ui/.node-version`). Run `npm ci` in `ui/` first; repeat it when the lockfile changes.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
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
