# Archive and stale: selected folded group

Open v3.html locally. The v1 folded group is selected; v3 records the confirmed
Idle wording and 1-to-365 range. v2 is retired. Use Switch theme or #light. Preview
controls use sample data only. They make no service, engine or network calls;
Reset reloads the fixture. Nothing is saved beyond the open page.

## Summary

```text
Existing Gathr shell
  Chat row / header More
    Archive -> confirm -> archived read-only conversation -> Restore
    Delete -> confirm -> another active chat, else New
  Archived
    v1: folded group after active chats
    v2: button above Settings, opening the shared navigation flyout
  Tasks flyout
    Questions
    Stale (human questions and agent tasks, each once)
  Workspace > Tasks
    Human question: existing answer pager
    Stale agent task: owner, state, reason, Restart and Cancel
  Settings > General
    Stale after: 3 days without progress
    Cancel after: 7 days since marked stale
```

Selected: v1, the folded group. It adds no destination and follows the existing
sidebar folds. v3 refines that layout with the confirmed copy and service range.
v2, the separate Archived flyout, is retired and must not be built. The original
v1/v2 pages and their preview.js snapshot remain unchanged for comparison.

### Selected design contract

- This is a refinement in Operate mode: variance 2, motion 2, density 5.
  Desktop use in daylight and at night keeps both existing themes. No new tokens.
- Use Public Sans, neutral canvas/surface, foreground actions and attention links
  from ../tokens.css. The signature remains the centred conversation and quiet
  chrome. No F11-specific accent, icon family or panel style.
- Compose the existing Sidebar, ChatMenu, registry DropdownMenu, shared Button,
  Dialog, Help, Notice, TasksFlyout and Workspace. Preview JavaScript is fixture
  code, never production code to port. A generic confirm is a variant of Dialog,
  with a wrapped question and action footer; do not add a second modal primitive.
- Row More appears on hover, selected row and keyboard focus, with an accessible
  chat-specific name. It does not select the row. Normal header menu keeps its
  Pause/Resume/Restart item; Archive follows it, and Delete sits after a separator.
  The preview's Pause toggle is only shell context, not a new pause specification.
- Confirm: "Archive <chat title>?" or "Delete <chat title>?", then Cancel and the
  verb. Title wraps, never truncates. Cancel receives initial focus. Escape
  cancels and restores focus. Archive uses the neutral primary Button. Delete
  uses the existing danger variant. No consequence paragraph.
- While a request runs, lock duplicate actions and expose a Saving/busy status.
  A failure keeps the confirm open with "Archive failed" / "Delete failed" and
  a Diagnostics reference. Keep the chat and its work visible until success.
- An archived chat opens read-only. Replace its composer with "Archived" and
  Restore; the same header/row menu offers Restore and Delete. Restore has no
  confirm and returns the chat to its original project or Chats. It never
  restarts cancelled tasks or waits. A Restore failure keeps this view and uses
  the shared referenced Notice. Archive/Delete effects are owned by the service.
- Removing the current chat selects the next active chat in the same group,
  else the previous one, else New. Removing another chat leaves the current
  draft and reading position alone. The fixture uses its first active chat
  after deletion; production should preserve sidebar adjacency as above.
- v1's Archived group starts folded and uses the chat-list scroll region. Keep
  its fold while the window is open. There is no separate archive flyout.
- Tasks keeps its existing count: open human questions, including stale human
  questions once. Agent tasks do not inflate it. Questions is the non-stale group;
  Stale follows it, expanded. Its own count includes all of its visible rows.
  Source says "For you" for human questions and the assignee for agent tasks.
  Show the chat and time since last progress as "Idle 4 days", plus a real hold/state for an agent.
- Picking a stale human question opens the existing exact-question pager, keeping
  Back/Next/Send and per-question drafts. The fixture shows one text answer to
  demonstrate this state, not a replacement for F9's full form set or batching.
- Picking a stale agent task opens its chat's Tasks tab with task title, giver,
  owner, state, reason and stale age. Restart is available only if the service
  says restartable; it is disabled for the sample open CI hold. Cancel acts on this task; the service retains its restartable record.
  Both use shared Button/Notice. Never show a
  person's answer form for an agent task. A cancelled record reached in the
  chat must remain inspectable and restartable where the service permits it.
- Stale does not mean abandoned: CI, reviews, open waits and running commands
  can keep work open. Never invent an expiry countdown for those. Human questions
  remain answerable until answered or explicitly cancelled by the person.
- In General, reuse settingrow and number inputs. Both are whole days, from
  1 to 365, with defaults 3 and 7. The second unit is "days stale", not total idle days.
  The ? detail explains progress and exceptions. Save on a committed edit;
  retain invalid input with "Enter whole days from 1 to 365.". Referenced save
  failures keep the edit visible. No helper paragraph under either setting.
- Reference geometry: 246px sidebar; 34px chat rows; 46px header; 32px pill
  text actions; menu 4px padding / 32px rows / quiet border; flyout 340px;
  Tasks workspace 480px (overlays below 1200px); Settings max 1024 by 760px,
  200px category rail. All use existing shared styles, not F11-specific copies.
- Preserve drafts and reading position on menus, confirms, Settings and list
  inspection. Focus uses the existing hover highlight, with no rings.
- Empty: "No archived chats", "No questions", "No stale tasks". Do not show
  engine plumbing or implementation detail in product copy.

### Selection and production handoff

The folded group is selected and recorded in DESIGN.md, system.html and 0015.
Carry these design files into the one F11 PR. Do not build the retired flyout
or start a separate production or design-system PR. The v3 adapter only refines
the accepted wording and range; the original proposal snapshot is frozen.
Use service state for stale eligibility, task permissions and all archive effects.
The approved slice-2 rules, not fixture timers, are the behavior authority.

## Evidence

- Selected-reference check: system.html and v3 at 1440/1024 in dark/light,
  compact confirm geometry, keyboard menu focus, Escape, Help, 365/366 range
  handling, Idle copy, long-title truncation and 63 archived rows. Eight page/size/
  theme combinations passed with no page errors. Shared Help checks also passed
  at both desktop widths and at 900px and 600px heights, in both themes.

- Before: now.html models Sidebar, TasksFlyout and General as of the F10 main,
  using invented content. It is a focused reference, not a running app capture.
- After: v1.html and v2.html exercise the requested F11 surfaces. Large data
  is 63 archived chats and 33 tasks. Normal data also includes long chat titles,
  a stale human question, a blocked agent task and an open-CI exception.
- Checked in installed Edge: all three pages at 1440 and 1024px, light and dark;
  each option at 390px, dark. Archive/Delete confirms, failure retention, restore,
  exact stale-task routes, human text entry, Settings help, Escape, validation,
  scroll bounds, empty states, design IDs and page errors were exercised.
- Serenity design checks reported no findings. Proof images and the check record
  stay outside this public repository. The companion check command is below.
- Not checked: real service behavior, engines, macOS/WebKit, screen readers,
  cross-window live updates, persistence or timings of a production operation.
  The fixture has no provider login, terminal, actual Settings service or files.

After installing the UI's documented dependencies, from the repository root:

```text
node designs/archive-stale/check-selected.cjs <absolute evidence folder outside the repo>
```

## Merge Danger

**Door:** two-way. Only local design fixtures are added.

**Blast Radius:** designs. No runtime, account or stored-data changes.
