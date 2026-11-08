# Open at login: placement proposals

Unselected. Do not implement until a version is picked.

## Summary

```text
v1: Settings > General > Open at login       (recommended)
v2: Settings > Engines > Startup > Open at login
```

General puts application behaviour ahead of provider configuration. Engines
keeps the existing category list, but places startup below account and program controls.
Both use the existing Settings shell, type and theme tokens.

Proposed control contract for either version:

- One row: Open at login, This computer, switch. No helper subtitle or Save button.
- Off by default for a new installation. Read the actual local state when opening.
- One shared Switch component in production. Track 44 by 24 px, thumb 16 px,
  hit area 44 by 32 px (44 px high on touch layouts). Use accent on, raised off,
  control outline off, text thumb off, canvas thumb on. Focus uses the existing ring.
- Native button with role=switch, aria-checked, labelled by Open at login and
  described by This computer. Space and Enter toggle. Thumb position carries state
  alongside colour. No animation needed.
- Apply immediately to this computer, independent of accounts, agents and chats.
- Disable only during the pending write; announce Saving. On failure restore the
  previous state and show Change failed with the existing Diagnostics reference.
- Default Settings entry can stay Engines in F6; this proposal does not change it.

Once picked, add only the chosen pattern to DESIGN.md and the switch specimen to
designs/system.html in the F6 PR, using shared production code rather than this HTML.

## Evidence

- Before: now.html is the selected Accounts preview, with no startup setting.
- After: v1.html and v2.html are working placement previews. Accounts and startup
  are simulated. No OS setting, service, account or login is changed.
- node designs/check.cjs passed. Serenity design check found no issues on either option.
- Installed Edge: Space enables, Enter disables, Escape closes Settings, both versions.
- Inspected at 1440 and 1024 px in dark and light; 390 px reflow also captured.
- Screenshots are outside the public repo at gathr-work/reviews/startup-design.
- No installed-app or OS login behaviour was tested; that belongs to F6.

## Merge Danger

Door: two-way.

Blast Radius: previews.

No runtime code or approved design-system pattern changes in this proposal.
