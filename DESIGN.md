---
name: Gathr
description: A desktop conversation workspace for a team of agents.
tokens: designs/tokens.css
colors:
  frame: "#232323"
  background: "#1b1b1b"
  foreground: "oklch(.95 .002 106)"
  card: "#2c2c2c"
  popover: "#2f2f2f"
  primary: "oklch(.922 .002 106)"
  primary-foreground: "oklch(.215 .003 106)"
  secondary: "#2c2c2c"
  muted-surface: "#2c2c2c"
  muted-foreground: "oklch(.74 .004 106)"
  accent-surface: "#383838"
  destructive: "lab(63.7053% 60.745 31.3109)"
  border: "oklch(.985 .002 106 / 12%)"
  input: "oklch(.985 .002 106 / 15%)"
  ring: "transparent"
  attention: "#60a5fa"
typography:
  body: { fontFamily: "Hanken Grotesk Variable", fontSize: "15px", fontWeight: 450 }
  composer: { fontFamily: "Hanken Grotesk Variable", fontSize: "16px", fontWeight: 450 }
rounded: { sm: "8px", floating: "16px", composer: "24px", action: "9999px" }
spacing: { xs: "4px", sm: "8px", md: "16px", lg: "24px", xl: "32px" }
components:
  composer: { backgroundColor: "{colors.popover}", rounded: "{rounded.composer}" }
---

# Design

<!-- impeccable:design-map -->
Name: Gathr
Colors: frame #232323, background #1b1b1b, foreground oklch(.95 .002 106), card #2c2c2c, popover #2f2f2f, primary oklch(.922 .002 106), primary-foreground oklch(.215 .003 106), secondary #2c2c2c, muted-surface #2c2c2c (+7 more in the frontmatter)
Don't:
- Do not replace accepted architecture, membership, safety or lifecycle policy
- Do not claim the current production screens already match this reference.
Map: ui/src/room
Map: designs/prototype-reference.md
Map: designs/system.html
Map: docs/archive/decisions/0070-the-windows-shell.md
Map: docs/archive/decisions/0068-the-windows-colours.md
Map: designs/fonts/hanken-grotesk-latin-wght-normal.woff2
Map: designs/tokens.css
Map: ui/
Full: .impeccable/design-full.md (read a section when you need its detail)
