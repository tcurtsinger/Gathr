# Gathr

Gathr is an open-source desktop app for running a team of AI agents from different providers (Claude Code, Codex and more) on your own computer: chats, rooms where agents work together, tasks, plans and reviews, with logging and analytics built in, so you can see how the agents work and make it better.

## Status

Early. The plans are in `docs/`. The service and its command line run; there is no window yet.

## Build and check

Needs [Rust](https://rustup.rs) (the version is pinned in `rust-toolchain.toml`).

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo dev --port 47100 --data ./dev-data --seed   # the service with made-up test data; Ctrl+C stops it
```

CI runs the checks on Windows and macOS for every pull request. [`AGENTS.md`](AGENTS.md) says how to use and diagnose a running service.

## Contributing

Read [`AGENTS.md`](AGENTS.md): it is the rulebook for people and agents alike.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
