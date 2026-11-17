# 0068: The window's colours

Status: accepted (owner, 2026-11-17: in dark, the outside shell lightest, the panels second, the chat darkest). Date: 2026-11-17.

Built: yes. Proof: `designs/tokens.css`, `designs/check.cjs`. No test: a colour is proved by pictures, and the design check holds the dark values.

Amends [0036](0036-surfaces-in-three-steps.md), whose surfaces and light theme stay as they are.

## The choice

- **In dark, the chat is darkest, the sidebar and the side panel next.** `--background` (the chat) is `#1b1b1b` and `--frame` (the sidebar, the top bar and the side panel) is `#232323`: 0036's two values, swapped.
- **Everything else stays.** Cards (`#2c2c2c`), floating surfaces (`#2f2f2f`), a menu's highlight, text, borders and tints keep their values. Tints and mixes made from `--background` or `--frame` follow them, so each still lifts or cuts out the surface it lies on.
- **Light doesn't change.** It already runs this way: a light-grey frame around a white chat.
- **The shell's own colour comes with the frame,** the later change that draws the window's outside shell, its rail and the rounded frame. Nothing uses a shell colour yet, so none is added here.

## Why

The owner's order for dark: the outside lightest, the panels second, the chat darkest. Today's dark had it the other way round, the chat lighter than the sidebar and the panel around it. With the chat darkest, cards, the composer and popovers stand further out from it, and the sidebar and the panel read as one frame around it.

## Turned down

- **Today's order** (sidebar and panel `#1b1b1b`, chat `#232323`): the opposite of what the owner asked for.
