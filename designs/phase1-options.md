# Phase 1 options

Working proposals, not changes to the selected design. Open each page locally; `#light` selects light mode. All data is made up. No engines, Settings service or clipboard are connected.

## Centered Settings revision

`engines/v3.html` keeps the v1 engine form in a centered Settings dialog over the current chat.
It adds a Settings header, Engines category and Close control. Escape closes the panel,
returns focus to Settings and retains the underlying draft. Engine fields remain when reopened.
The panel is 1024 px wide at most, with content scrolling inside it. Narrow windows stack
the category above the form. Gathr's existing tokens, controls and engine preview states apply.

Operate mode: variance 2, motion 1, density 6 for Diagnostics and 4 elsewhere. Existing tokens, type, shell and shared control styles apply. Content aligns left. No new palette or decoration.

```
Diagnostics 1: sidebar | turn list | selected timeline
Diagnostics 2: sidebar | chronological turn disclosures
Engines 1:     sidebar | one engine form
Engines 2:     sidebar | engine list | engine form
Chat states 1: sidebar | transcript with turn notice | composer
Chat states 2: sidebar | transcript | notice above composer
```

- **Diagnostics: v1, selected turn.** Keeps 48 turns scannable while a 50-call turn is expanded. v2 keeps the chronology in one scroll, but expanding several turns pushes navigation away.
- **Engines: v1, single form.** One engine needs one page in this slice. v2 gives engine selection its own column, which is useful with multiple engines but costs width now.
- **Chat states: v1, beside the reply.** A stopped reply keeps its state with its retained text. A failed send and a held message keep their state beside the request. v2 keeps recovery near the composer but detaches it from older turns.

## Patterns proposed for the chosen design

One neutral notice with a state word and recovery action. Failure references are links styled as buttons: production copies on click; these prototypes show copy feedback without accessing the real clipboard. Diagnostics opens the matching reference and cause. Stopped text remains intact; Continue prefills the composer. A held message opens Engines for sign-in. Successful sign-in releases held messages in production, per first-slice.md; navigation here does not persist state between pages.

Provider token totals retain their labels. Gathr context estimates are disclosed separately and explicitly marked. A failed engine start shows tokens as not reported. No inferred split of engine and conversation history.

Settings state controls and the chat state selector are preview controls, removed from production. The sign-in dialog simulates completion only. Path Save produces local feedback only; missing paths are selected as a preview fixture, not tested against the filesystem.

## Checks

`node designs/check.cjs` checks the existing selected pages plus the six proposals. Browser proof and interaction results are kept outside the public repo in the room's evidence folder. These pages do not replace DESIGN.md or system.html; those change after a direction is chosen.
