# 0035. Search: an FTS5 index of message text, word prefixes, newest first

Status: accepted (the owner's picks D1 to D3, 2026-11-10). Date: 2026-11-10.

Built: yes. Proof: `crates/gathr/tests/search.rs`, `ui/src/search/SearchFlyout.test.tsx`.

## Choice

- **Index:** migration 0015 adds `message_search`, an SQLite FTS5 table over `messages.body` with external content
  (no second copy of the text), tokenizer `unicode61 remove_diacritics 2`, so case and accents fold ("cafe" finds
  "Café"). Only `kind = 'text'` rows are indexed, whatever the role; tool rows are JSON nobody searches for. Three
  triggers keep it in step with every insert, body change and delete. FTS5 comes with our bundled SQLite; no new
  dependency.
- **What a word matches:** each typed word is quoted, so nothing typed is query syntax (`"`, `*`, `-`, `:`, `NEAR`,
  `AND`), and matched as the start of a word; every word must match. A word with no letters or digits drops out.
- **Order:** newest first by rowid, the order messages were saved.
- **Titles:** a plain `LIKE` substring over chat titles, at most 8, most recently active first.
- **Archived chats** are in by default, each marked; `archived=false` leaves them out.
- **Snippets** come with `marks` in UTF-16 units, as the live socket's `at`; the service never sends HTML.
- **Events:** one `gathr.search.query` per request with its size and hits, never the words searched.

## Why

Measured on this PC with 20 seed runs (11,460 text messages, 9.3 MB) and a 10x copy (114,600, 92.6 MB), release
build, the service's own `duration_us`, median of 10:

| Query | 11k | 115k |
|---|---|---|
| a common word (`river`), 30 newest | 0.8 ms | 3.1 ms |
| a 3-letter prefix (`her`) | 0.7 ms | 3.7 ms |
| 4 words, all required | 1.5 ms | 11.7 ms |
| no match | 0.3 ms | 0.3 ms |
| 100 hits | 1.3 ms | 3.9 ms |

Migration 0015 builds the index in about 0.8 s at 115k messages, and adds 25 MB to a 92 MB text history.

## Turned down

- **`LIKE '%q%'` over every message** (as SerenityOS's search does): 60 to 140 ms per keystroke at 115k messages,
  reading everything every time, slower as history grows.
- **The `trigram` tokenizer** (matches inside words, "rav" in "gravel"): an index twice the size of the text and a
  migration 7 times slower, for a match people rarely type (owner's D2).
- **Ordering by message time:** sorts every match, 15 to 20 times slower than rowid order at 115k.
- **Ranking by relevance (bm25):** newest first is predictable and cheap; it can come later if it proves wrong.

## Rules this sets

- Never run FTS5's `'rebuild'` on `message_search`: it would index the tool rows too.
- A migration that rebuilds `messages` keeps every rowid (`INSERT INTO messages_new (rowid, …) SELECT rowid, …`) and
  refills the index. `tests/search.rs` fails if index and messages drift apart, and checks that `VACUUM INTO`, which
  the update's backup uses, keeps the rowids.
