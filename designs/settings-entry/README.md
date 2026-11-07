# Settings entry and managed engines

Local design options, with no service, downloads or sign-in connected. Open each
HTML page directly. `#light` starts the light theme. Preview controls above the app
simulate reply and update states. All data is sample data.

- `now.html`: the existing entry, header Diagnostics link and visible path form,
  reconstructed from Sidebar.tsx, ChatView.tsx and Engines.tsx at 0cdd0f5. Sample
  transcript and preview controls are added; this is not a captured running app.
- `v1.html`: labelled Settings row at the sidebar foot; path under Advanced.
- `v2.html`: local workspace menu at the sidebar foot; engine source has Change.
- `v3.html`: gear beside Gathr; managed/custom choice under Engine source.

The Settings entry and Engines treatments can be chosen independently. All options
retain the centred Settings panel, category rail, version, update state and sign-in.
Diagnostics is reached from a reply reference or Settings, with no chat header link
or chat menu item. The sample Diagnostics page only demonstrates navigation.

## Design contract

Refinement of Gathr's existing Operate surface. Variance 2, motion 1, density 4.
Use the shipped tokens and base controls through `ui/src/base.css` and the shipped
Settings shell through `ui/src/settings/settings.css`. Charcoal canvas #191919,
surface #222222, text #e8e8e6, muted #b0b0ad and focus #a6c4df come from the shared
tokens; warm light overrides remain shared. Segoe UI Variable, 15 px prose, 13 px
navigation, 18 px section titles. No new tokens or assets.

The deliberate detail is the labelled, left-aligned Settings row in option 1.
It is directly discoverable and adds no workspace or account concept. The custom
path stays accessible for testing, but outside the default engine controls.

```
Sidebar                 Conversation
  Gathr                  Chat title
  New                    Messages
  Chats                  Composer
  Settings

           Settings (centred dialog)
           Categories | Engines
                      | Claude Code / source
                      | Version
                      | Update state / Check or Update
                      | Login state / Sign in
                      | Advanced
```

Custom paths replace the managed engine and show that updates are external.
Clear and save a path to return to Gathr's copy. Preview controls can reset the
simulated update state independently. No account sign-out action is proposed.
The local workspace menu does not represent a Gathr login.

## Check

Run `node designs/check.cjs` for the existing preview suite. Open these pages at
1440, 1024 and 390 px, in dark and light. Open Settings (for option 2, first open
Local workspace), change categories, close with Escape, and check the draft stays.
Try each custom path disclosure, Save, and clearing the path. Use the preview
controls to show failed/stopped replies and update states; open their references.
These are prototype checks, not proof of engine integration or chat performance.

After installing the UI dependencies, run `node designs/settings-entry/source.check.cjs`
on Windows (uses the installed Edge). It checks source switching through the real
page controls, Search at desktop sizes, and Settings/Escape at all three sizes in
both themes. `GATHR_EVIDENCE` optionally names a screenshot output folder.

Keep DESIGN.md and the shared system unchanged until a direction is selected.
