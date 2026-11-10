# 0023: Settings and reply states

Status: accepted.

Settings opens centered over the current screen, with categories at the left and
an X at the top right, named Close Settings in its tooltip and to assistive tech. Engines uses one form for the first engine. Diagnostics
is a category, with a turn list beside the selected timeline (option 1).
Chat Diagnostics links and failure references open the matching turn in this panel.

Amended 2026-11-10: the owner picked the X over the text Close (UX pass, item 8). It
matches the prototype's dismissal and keeps the header quiet; the text Close was the
option turned down. Other dialogs keep their text Close.

Stopped, waiting for sign-in and failed-send notices sit where the reply belongs.
Failures use a red tinted surface with Send failed, the reference, Diagnostics and
Retry at the right. Stopped and waiting use a neutral surface. Retain partial text
and drafts. Sign in opens Engines in the same panel.

This keeps recovery beside the affected message and preserves the current screen.
Turn navigation stays available while a long timeline scrolls.

Turned down: standalone Settings and Diagnostics pages (lose the current screen),
a chronological Diagnostics accordion (long turns push navigation away), and a
notice above the composer (detaches the state from its message).

See DESIGN.md and designs/system.html for tokens and components. Working selected
previews are listed in designs/README.md. No backend behavior is simulated as real.
