# 0036. Colours: surfaces in three steps, in both themes

Status: accepted (the owner's pick, Theme A, 2026-11-11). Date: 2026-11-11.

Built: yes. Proof: `designs/tokens.css`, `designs/check.cjs`. No test: the tokens file is the decision, and the design check holds the three dark values.

## Choice

- **Three surfaces, back to front:** `--frame` (navigation, the top bar, the workspace), `--background` (the
  chat), `--card` (what stands on the chat: an approval, a notice). Floating surfaces (the composer,
  popovers, the one dialog, Help) use `--popover`. `--secondary` and `--muted-surface` (the person's message)
  follow the card.
- **Dark:** frame `#1b1b1b`, background `#232323`, card, secondary and muted surface `#2c2c2c`, popover
  `#2f2f2f`, a menu's highlight (`--accent-surface`) `#383838`. Text is `oklch(0.95 0.002 106)`, a little
  under white; muted text is `oklch(0.74 0.004 106)`. Borders are `oklch(0.985 0.002 106 / 12%)`.
- **Light:** background and popover are white; frame, card and secondary are `oklch(0.965 0.003 106)`; the
  muted surface is `oklch(0.955 0.003 106)` and a menu's highlight `oklch(0.935 0.003 106)`; borders are
  `oklch(0.895 0.005 106)` and input borders `oklch(0.86 0.005 106)`. Muted text is `oklch(0.5 0.007 106)` and
  attention `#1d4ed8`, so both pass 4.5:1 on every surface, the selected navigation row included.
- **`--raised` and `--danger-surface` are tints** (6% of the foreground, 10% of destructive, over transparent),
  so a hover, a selected row or an error lifts whatever surface it lies on.
- **The Tasks count** in navigation is the attention colour while tasks wait on the person.
- The values live in `designs/tokens.css`; `DESIGN.md` describes them.

## Why

The owner found light mode hard to read and dark mode too dark. In light, the page, the cards and the popovers
were one near-white (`oklch(0.992 0.002 106)`) with a pale border, so nothing had a surface of its own. In dark,
the page was `#181818` and every other surface `#202020`, under near-white text.

## Turned down

- **Theme B, a lifted dark and a paper light** (mid-grey dark; light-grey chat with white cards). Easier on the
  eyes in dark, but its light theme turns cards white on grey, which every screen would need redrawing for.
- **Keeping the values and only strengthening borders.** It separates regions but not surfaces: a card and the
  page would still be the same colour.
