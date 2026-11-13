# 0027. Reply formatting: react-markdown with remark-gfm, re-parsing only the end while streaming

Status: accepted, renderer replaced by [0030](0030-ui-foundation.md) (Streamdown); raw HTML as text and unsafe links dropped still hold. Date: 2026-11-07. Chosen by the lead from the frontend's options.

Built: partly. Proof: `ui/src/chat/Thread.test.tsx`, `ui/src/components/Markdown.test.tsx`. Built: raw HTML shown as text, unsafe links dropped, only the end parsed again while streaming. Not built as written: the react-markdown renderer, which [0030](0030-ui-foundation.md) replaced with Streamdown.

Out of date below: the timings are the old renderer's; code highlighting and Copy exist now ([0030](0030-ui-foundation.md)).

## Choice

| Job | Pick | Version checked | Licence |
|---|---|---|---|
| Markdown to React elements | `react-markdown` | 10.1.0 | MIT |
| Tables, task lists, strikethrough, autolinks | `remark-gfm` | 4.0.1 | MIT |
| The streaming reply's block tree | `unified` with `remark-parse`, the parser react-markdown uses | 11.0.5, 11.0.0 | MIT |

Versions are the npm `latest` on the date above. The lockfile pins them. `unified` and `remark-parse` were already installed under react-markdown; listing them directly adds no download.

- **One shared `Markdown` component** (`ui/src/components/Markdown.tsx`) formats an agent's text. User messages stay plain text.
- **A saved reply** renders exactly as react-markdown renders its whole text.
- **A streaming reply** is split into its top-level blocks as the parser sees them (`blocks.ts`). Each block renders through react-markdown, memoized on its text, so only the blocks that changed render again. A list is one block, so its numbering holds. Each block starts with the document's link definitions, so its references resolve; they go first because a code fence still open at the end would take in anything after it. A reply with footnotes renders whole, because a footnote's definition renders apart from its reference. So does a reply with a link or footnote definition inside a quote or list (Vale, 938): such replies are rare, so whole rendering costs little and stays correct.
- **Re-parsed from the second-to-last block:** while the text only grows, blocks before the last two can't change, so each update parses from where the second-to-last block starts. Not the last block alone: a paragraph "1" after a list becomes "1." and joins that list.
- **The test** (`Markdown.test.tsx`) streams each input into one component, a character at a time, and compares its HTML with the whole text's at every length: Vale's three inputs (a numbered list, a reference link, a fence with trailing text), a footnote, a setext heading, a list with a continuation, a table forming, a lazy quote, a fence with blank lines, a list after text, and all of them in one reply.
- **Links** open in the person's browser: each gets `target="_blank"`, which the desktop window sends to the browser (PR #30).
- **Raw HTML stays text** and unsafe link schemes (`javascript:` and the like) are dropped: react-markdown's defaults, with no HTML plugin added. A test holds both.

## Why

- **React elements, not an HTML string:** nothing is set as raw HTML, so there's no sanitizer of our own to keep right, and each element can use Gathr's own components.
- **Size:** the UI's script grows from 76 to 126 KB gzipped (measured with `vite build`). That's loaded once, from disk, in a desktop app.
- **Streaming, measured:** a made-up 20,150-word Markdown answer (headings, lists, code, tables, quotes) streamed in 2,641 deltas over about 28 s through the replay engine, in Edge at 1440 px.

  | While streaming | p95 frame | Frames over 50 ms | Worst frame |
  |---|---|---|---|
  | One react-markdown over the whole text (dev build) | 133 ms | 201 | 206 ms |
  | Whole parse, changed blocks re-render (dev build) | 109 ms | 195 | 176 ms |
  | Whole parse, changed blocks re-render (production build) | 36 ms | 1 | 61 ms |
  | Parse from the second-to-last block (dev build) | 6.2 ms | 1 | 218 ms |
  | Parse from the second-to-last block (production build) | 6.1 ms | 1 | 85 ms |

  A whole parse takes, in Node, 13 ms at 30,000 characters, 24 ms at 59,000 and 50 ms at the full 119,000, so it couldn't run on every update. The one slow frame left is the end of the stream, when the saved reply replaces the streaming one and renders whole once.
- **Opening a chat** costs more, because every reply is now parsed. In a production build, median (worst) of ten opens: the 500-message chat 39 to 95 ms (122), the 20,000-word answer 50 to 57 ms (60), the streamed Markdown answer 50 to 107 ms (166). That's inside the first slice's 200 ms.

## Turned down

- **`marked` with DOMPurify:** smaller (not measured here), but it produces an HTML string, which needs a sanitizer and patching afterwards to use our components.
- **`markdown-it`:** the same HTML-string trade-off.
- **Splitting the text at blank lines** and parsing each piece alone (the first version of this PR): fast, but it changed what the Markdown means. A numbered list restarted at 1 and a reference link didn't resolve (Vale's review).
- **Raw HTML through `rehype-raw`:** agents' HTML has no use in a reply that a code block doesn't serve, and it would need a sanitizer.

## Checked, and not yet

- **Syntax highlighting** in code blocks: not added. It's its own choice, with its own cost.
- **Copy on code blocks:** not designed yet.
