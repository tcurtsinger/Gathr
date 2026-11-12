# 0036. Colours: surfaces in three steps, in both themes

Status: accepted (the owner's pick, Theme A, 2026-11-11). Date: 2026-11-11.

## Choice

- **Three surfaces, back to front:** `--frame` (navigation, the top bar, the workspace), `--background` (the
  chat), `--card` (what stands on the chat: the person's message, an approval, a notice). Floating surfaces
  (the composer, popovers, the one dialog, Help) use `--popover`.
- **Dark:** frame `#1b1b1b`, background `#232323`, card `#2c2c2c`, popover `#2f2f2f`. Text is
  `oklch(0.95 0.002 106)`, a little under white. Borders are 12% of the foreground.
- **Light:** background and popover are white; frame and card are `oklch(0.965 0.003 106)`; borders are
  `oklch(0.895 0.005 106)` and input borders `oklch(0.86 0.005 106)`.
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
