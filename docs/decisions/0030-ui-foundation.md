# 0030. The chat on assistant-ui, Streamdown and Tailwind, with the registry files kept as published

Status: accepted (packages approved by the owner, 2026-11-08). Date: 2026-11-08. Replaces [0027](0027-reply-formatting.md)'s
renderer. Amends [0022](0022-ui-tooling.md)'s "no component library". The owner chose assistant-ui's components for the chat
screens, and the design reference is the assistant-ui prototype (DESIGN.md).

## Choice

| Package | Version | Licence | Why |
|---|---|---|---|
| `@assistant-ui/react` | 0.15.33 | MIT | Thread and message primitives, the runtime over Gathr's own state |
| `@assistant-ui/react-streamdown` | 0.3.21 | MIT | A reply's text part, rendered by Streamdown |
| `streamdown` | 2.7.1 | Apache-2.0 | Streaming Markdown |
| `radix-ui` | 1.8.1 | MIT | Collapsible, tooltip and slot under the registry files |
| `lucide-react` | 1.70.0 | ISC | Icons; only the ones used reach the bundle |
| `class-variance-authority` | 0.7.1 | Apache-2.0 | Variants in the registry files |
| `cn` | 0.5.2 | MIT | `@/lib/utils`'s `cn`, as the registry files import it |
| `tw-animate-css` | 1.4.0 | MIT | Animations the registry classes name |
| `tw-shimmer` | 0.4.14 | MIT | The registry's running-state shimmer |
| `tailwindcss`, `@tailwindcss/vite` (dev) | 4.3.5 | MIT | Build-time CSS for the registry and Streamdown classes |

Pinned exactly. Installed from this PC's npm cache, so nothing new was downloaded. `react-markdown`, `remark-gfm`,
`remark-parse` and `unified` are no longer direct dependencies. The approved `@fontsource-variable/public-sans` isn't
needed: `designs/tokens.css` already self-hosts the font.

- **Registry files** live in `ui/src/registry`, in the prototype's tree (`components/…`, `lib/utils.ts`), imported as `@/…`.
  Only what a merged screen uses comes in: 14 files for the chat. They stay as published: `ui/registry.json` records each
  file's source URL and sha256, and `node scripts/registry.check.mjs` (in CI) fails on a changed or unrecorded file. A change
  that has to go inside one is an adaptation: its new hash and why go in the same record. Two so far, both from the prototype
  and both recorded: Button's 32px pill actions, and SpeakerIdentity's centring plus Gathr's initials in place of the robot icon.
  Biome and the 1,000-line check skip the folder (`tool-fallback.aui.tsx` is 1,330 lines); everything else in `ui/` keeps both.
- **Tailwind** is one entry, `ui/src/theme.css`: tokens, Tailwind, the registry's CSS records (`registry/registry.css`), then
  Gathr's `base.css` in Tailwind's base layer. `@theme inline` maps the shadcn names to Gathr's tokens; `muted` and `accent` read
  `--muted-surface` and `--accent-surface`, because Gathr's own `--muted` and `--accent` are text colours. The reset clashed
  twice on existing screens, and both are fixed: a modal dialog's centring, and the Help title's weight. `base.css`'s element
  rules skip controls with `data-slot` (every registry control has one), so the registry's own classes win.
- **No focus rings** (the owner's rule): `theme.css` clears `outline` and `--tw-ring-color` on `:focus-visible`, unlayered, so it
  beats the registry's utilities without editing them (most name their ring colour in the class, not `--ring`). Keyboard focus
  lays the hover tint over the control's own background instead.
- **The runtime:** `useExternalStoreRuntime` over Gathr's `ChatState`. `useChat` and `/api/live` stay. `chat/messages.ts`
  converts: each person's message, then one assistant message per turn with its text and tool calls in order, its streaming
  text last, and its notice and approvals in `metadata.custom`. A reply's id is `a-<turn>` from its first streamed text to its
  saved messages, and an unchanged reply keeps its object, so a delta converts one message, not 500.
- **The composer** is Gathr's own (`components/Composer.tsx`) built from the registry's composer parts, so drafts per chat,
  Enter and Shift+Enter, and Stop stay as they were. The agent picker is the registry's menu.
- **Notices** (stopped, waiting for sign-in, failed) stay Gathr's `Notice` inside the reply: the registry's ErrorState and
  StoppedRun have no place for a reference or Diagnostics (0019, 0023). **Approvals** use the registry's ApprovalCard.

## Streamdown, set for Gathr

- **Raw HTML is shown as text, never parsed or run:** a remark step turns every HTML node into text before rendering. Then
  `rehype-raw` and the sanitizer have nothing to do, so only `rehype-harden` runs, which still drops unsafe link and image URLs
  (`javascript:` reads "[blocked]").
- **Links** open in the person's browser (`target="_blank"`), Streamdown's link-safety dialog off. In-page links (footnotes)
  stay in the reply.
- **Code** renders as plain `pre` and `code`, as under 0027. Streamdown's own code block loads its body lazily, and React holds a
  revealed Suspense fallback for 300 ms, which put a chat with code 300 ms late on first open. Table controls (copy, download,
  fullscreen) are off.
- **Finished replies** parse in one pass (Streamdown's static mode). A reply drawn while streaming stays in streaming mode
  until it's drawn again, so a stream's end never re-parses it whole.
- **Long chats** draw their newest 100 messages; scrolling near the top adds the next 100 above, keeping the messages on screen
  in place (`chat/older.ts`). A page search (Ctrl+F) only finds what's drawn.

## Timings

Edge, 1440 × 900, production builds on the same service and data, measured 2026-11-09 (`pia-format`: 500 messages with 147 tool calls, a
20,000-word answer, 50 tool calls in a turn), 10 rounds. Budgets from `slice-2b.md`.

| | `main` | This | Budget |
|---|---|---|---|
| Cold start to the newest chat drawn (median of 10, max) | 134 (148) ms | 201 (233) ms | 300 ms |
| Switch to the 500-message chat (median, max) | 74 (79) ms | 69 (85) ms | 200 ms |
| Switch to the 20,000-word answer | 35 (37) ms | 43 (48) ms | 100 ms |
| Enter to the reply's first text | 123 ms | 107 ms | 150 ms |
| Worst frame while a 20,000-word reply streams | 60.6 ms | 6.2 ms | 50 ms |

## Turned down

- **Rewriting the registry components in plain CSS:** about 20,000 lines to own, and every upstream fix a hand port.
- **Splitting or reformatting registry files to pass our limits:** turns every update into a hand merge.
- **Raising the line limit for everyone:** it would loosen our own code too.
- **Streamdown's raw HTML, sanitised:** 0027's rule stands; an agent's HTML has nothing a code block doesn't serve.
- **The prototype's ComposerPrimitive input:** a single-line `<input>`, and its draft lives in the runtime, not per chat.
- **The registry's ErrorState and StoppedRun for notices:** no reference or Diagnostics link.
