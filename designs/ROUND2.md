# Round 2 candidates

Nine local working pages, v4 through v6. Earlier versions are preserved. All activity is fictional. No imported interface, fonts or assets. Open a matching version of overview, room and chat in the design viewer. Dark switches the paired theme; links preserve it.

Subject: supervising a local team of agents across providers. Audience: someone managing concurrent project work at a desktop in daylight or evening light. Job: see who is working, resolve a wait and continue the conversation. Mode: Operate.

| Version | Visual language | Identity | Dials: variance, motion, density |
| --- | --- | --- | --- |
| v4 Signal | Restrained forest neutrals, Segoe UI, compact rows, 3px corners, dark navigation | Stable geometric marks for the five room members, monograms for additional members | 2, 1, 8 |
| v5 Folio | Restrained blue neutrals, Georgia headings with Calibri body, open transcript, 5px controls | Framed two-letter monograms | 2, 1, 5 |
| v6 Relay | Plum rail, tinted neutral canvas, Trebuchet headings and Verdana body, 16px grouping, pill actions | Abstract faces with stable silhouettes and expressions; monograms for additional members | 3, 1, 6 |

Pick: v4 Signal. Work states scan fastest and leave the most space for conversation. Folio prioritizes reading, Relay gives the team a friendlier presence at a higher space cost. None animates activity without a live source.

Candidate token roles are canvas, surface, soft, ink, muted, line, accent and selected. They are embedded in each page, with paired light and dark values. The chosen system will extract these into shared tokens and components. Per the approved Phase 0 plan, DESIGN.md and designs/system.html follow the owner's pick, rather than declaring a candidate canonical.

Body is 16px. Metadata is 13px, states 12px, task rows 14px. Titles are 30px (Signal), 36px (Folio) and 32px (Relay). Spacing uses 4px steps. Reading lines stay within 70ch. Alignment is left throughout.

Layouts:

    Signal: rail | title + actions
                 conversation | team, tasks, plan check
    Folio: brand + horizontal navigation
           title + actions
           open transcript | team, tasks, plan check
    Relay: plum rail | title + actions
                      soft conversation | rounded work groups

The signature is the state treatment: Signal uses a solid working mark, a dotted wait, a filled plan check and pause bars. Folio uses an action-colored underline for Working and dotted Waiting. Relay fills Working as a capsule while waits stay open. All states have text; identity never encodes provider or activity.

Each room and chat contains 18 expanded message entries with successful and failed tool runs, a hand-off, a three-paragraph answer and an attached side job. The history scrolls normally; it is not hidden behind an earlier-messages disclosure. The count matches the fixture. Overview has twelve agents. Chat uses a stable Mira identity and a separate You monogram.

Use: navigate among the three screens; switch Dark/Light; Pause/Resume; open tool, side-job and task disclosures; open Review then Close; type and Send; remove the queued chat message. Sending appends escaped local text and clears the draft. These controls do not persist or contact a provider. Review only previews a plan, with no approval operation. Empty sends keep focus in the draft. The narrow layout stacks work details below conversation and retains touch targets.

Check from the repo root:

    node designs/check.cjs
    node designs/check-round2.cjs

Then render all pages at 1440 and 1024 in light and dark, and at 390 for reflow. The private evidence gallery includes those captures, full transcript views and computed contrast results. No live engine, service, streaming, login, Tauri or production timings are claimed. No additional assets are required.
