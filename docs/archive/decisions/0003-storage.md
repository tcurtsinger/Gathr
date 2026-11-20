# 0003. SQLite through rusqlite, one writer thread, migrations as numbered SQL files

Status: accepted. Date: 2026-11-06.

Built: partly. Proof: `crates/gathr/tests/service.rs`. Not built: the read-only pool (only `gathr diag` reads on its own connection); the local-disk check at start; the migration test on a copy made by the previous release; spilling large blobs to files; online backup.

Out of date below: only migrations start with `BEGIN IMMEDIATE`; engine frames are stored as text, and only event attributes as JSONB; the read-only connection sets only its busy timeout.

## Implementation status

SQLite, WAL, migrations and the writer thread are built. API reads currently use the writer thread too; a separate read-only pool, large-frame blob spill and online backups are not built yet.

## Choice

- One SQLite database file, `<data>/gathr.db`, on a local disk, holding everything the service owns: agents, chats, messages, tasks and the event record (0011).
- Crate: [`rusqlite`](https://crates.io/crates/rusqlite) 0.41 with the `bundled` feature, so the SQLite version is the one we build with, not whatever the OS has.
- One writer: a dedicated thread owns the only write connection and takes work over a channel. Reads use a small pool of read-only connections on blocking threads (`tokio::task::spawn_blocking`).
- Pragmas on every connection: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `busy_timeout=5000`. Writes start with `BEGIN IMMEDIATE`.
- Migrations: numbered files `migrations/0001_<name>.sql` embedded with `include_str!`, applied in order inside one transaction each, with the applied number kept in `PRAGMA user_version`. The service refuses to open a database newer than it knows.
- JSON payloads (event attributes, engine frames) are stored as JSONB and queried with SQLite's JSON functions.

## Why

- **rusqlite is the plain, current choice.** 0.41.1 (2026-10-28, MIT) bundles SQLite 3.54.0 ([crates.io](https://crates.io/crates/rusqlite)). sqlx 0.9.0 bundles SQLite 3.51.3 ([crates.io](https://crates.io/crates/sqlx)), and its SQLite driver runs each connection on its own thread anyway, so its async API buys nothing over a writer thread of our own.
- **One writer matches SQLite.** SQLite allows one writer at a time; WAL lets readers run alongside it and the setting persists in the file ([WAL](https://sqlite.org/wal.html)). `BEGIN IMMEDIATE` takes the write lock at the start, so a wait happens there instead of failing mid-transaction ([transactions](https://sqlite.org/lang_transaction.html)). `synchronous=NORMAL` in WAL mode can lose the last commits on power loss but never corrupts ([pragma synchronous](https://sqlite.org/pragma.html#pragma_synchronous)). A queue in front of one connection makes write order explicit, which the event record needs.
- **Migrations in a few lines.** `user_version` is a free integer in the database header ([pragma user_version](https://sqlite.org/pragma.html#pragma_user_version)). A loop over embedded files is about thirty lines with a test; AGENTS.md says not to add a dependency for what a few lines can do. It also avoids a version lock: rusqlite_migration 2.6.0, refinery 0.10.0 and tokio-rusqlite 0.8.0 all still pin rusqlite 0.40, and only one `libsqlite3-sys` can be linked into a build ([rusqlite_migration](https://crates.io/crates/rusqlite_migration), [refinery](https://crates.io/crates/refinery), [tokio-rusqlite](https://crates.io/crates/tokio-rusqlite)).
- **JSON is built in.** JSON functions are in SQLite's core since 3.38 and JSONB since 3.45 ([json1](https://sqlite.org/json1.html)), both below the bundled 3.54.
- **One file replaces six.** The previous app kept its record in `events/`, `native/`, `usage.json`, `engine-turns.json`, `record.ndjson` and `server.log`. One database makes the diagnostics page a query.

## Rules that follow

- WAL doesn't work on network drives ([WAL](https://sqlite.org/wal.html)): the data folder must be local, and the service says so at start if it isn't.
- Each migration ships with a test that applies every migration to an empty database and to a copy made by the previous release.
- Large engine frames (file contents, long tool output) go in the database up to a size cap; beyond it they are written to `<data>/blobs/` by hash and referenced. The cap is set in the first slice from real frames.
- Backups use SQLite's online backup API, never a file copy of a live database.

## Turned down

- **sqlx.** Older bundled SQLite, a second async layer over a thread per connection, and compile-time query checks that need a database or saved query data in CI.
- **diesel.** A synchronous ORM; the schema is small and the event record is append-mostly, so an ORM adds more than it saves ([crates.io](https://crates.io/crates/diesel)).
- **libsql.** In maintenance mode; its README sends new projects to Turso ([crates.io](https://crates.io/crates/libsql)).
- **rusqlite_migration or refinery.** Both fine, both pin rusqlite 0.40 today, and the job is thirty lines.
- **Files per chat (ndjson), as the previous app did.** Fast to append, but every question across chats becomes a scan of many files.
