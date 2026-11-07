# 0027. Reply formatting: react-markdown with remark-gfm, parsed a block at a time

Status: accepted. Date: 2026-11-07. Chosen by the lead from the frontend's options.

## Choice

| Job | Pick | Version checked | Licence |
|---|---|---|---|
| Markdown to React elements | `react-markdown` | 10.1.0 | MIT |
| Tables, task lists, strikethrough, autolinks | `remark-gfm` | 4.0.1 | MIT |

Versions are the npm `latest` on the date above. The lockfile pins them.

- **One shared `Markdown` component** (`ui/src/components/Markdown.tsx`) formats an agent's text, streaming or saved. User messages stay plain text.
- **Links** open in the person's browser: each gets `target="_blank"`, which the desktop window sends to the browser (PR #30).
- **Raw HTML stays text** and unsafe link schemes (`javascript:` and the like) are dropped: react-markdown's defaults, with no HTML plugin added. A test holds both.
- **Parsed a block at a time:** the text is split at blank lines outside code fences (`blocks.ts`), and each block renders through a memoized component. While a reply streams, only its last block is parsed again. A blank line before an indented line doesn't split, so a list item's continuation stays with its item.

## Why

- **React elements, not an HTML string:** nothing is set as raw HTML, so there's no sanitizer of our own to keep right, and each element can use Gathr's own components.
- **Size:** the UI's script grows from 76 to 126 KB gzipped (measured with `vite build`). That's loaded once, from disk, in a desktop app.
- **Block at a time, measured:** a made-up 20,150-word Markdown answer (headings, lists, code, tables, quotes) streamed in 2,641 deltas over about 28 s through the replay engine, in Edge at 1440 px. Parsing the whole text on every delta gave a p95 frame of 133 ms, with 201 frames over 50 ms. Block at a time gave a p95 of 6.2 ms, with one frame over 50 ms: when the saved message replaces the streaming one and parses once (73 ms in a production build).
- **Opening a chat** costs more, because every reply is now parsed. In a production build, median of ten opens: the 500-message chat 39 to 79 ms, the 20,000-word answer 50 to 45 ms, the streamed Markdown answer 50 to 97 ms. That's inside the first slice's 200 ms.

## Turned down

- **`marked` with DOMPurify:** smaller (not measured here), but it produces an HTML string, which needs a sanitizer and patching afterwards to use our components.
- **`markdown-it`:** the same HTML-string trade-off.
- **Parsing the whole reply on every delta:** the stalls measured above.
- **Raw HTML through `rehype-raw`:** agents' HTML has no use in a reply that a code block doesn't serve, and it would need a sanitizer.

## Checked, and not yet

- **Syntax highlighting** in code blocks: not added. It's its own choice, with its own cost.
- **Copy on code blocks:** not designed yet.
