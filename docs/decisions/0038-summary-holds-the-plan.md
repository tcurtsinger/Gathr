# 0038. Summary holds the plan and the team

Status: accepted (owner, 2026-11-11, on the chat redesign's approved design). Date: 2026-11-11.
Replaces 0015's plan bar above the composer and its Summary of expandable rows.

## Choice

- **No plan bar and no status bar.** Summary says what both would. A chat with more than one agent has it.
- **Summary's button is initials only.** In the chat header: the overlapped initials of every agent with a
  running turn, each with a turning attention ring (a still ring under reduced motion). Whoever needs the
  person, through a waiting approval or a question it asked them, sits last in the stack with no ring, and
  one attention dot sits on the stack's corner. With nobody working or needed it is a plain icon. Its
  accessible name and tooltip say it in words: "Summary: Vale needs you, Juno and Pia working".
- **The plan is in Summary.** The Supervisor's current round (its newest hand-off's tasks, as before) heads
  the popover: its segments and "1 of 3 done", then one row for each step with its state, its title and who
  has it, marked "needs you" while that agent waits on the person.
- **A finished round leaves with the person's next message.** Once every task is settled and the Supervisor
  has heard back, the round stays until the person writes again; then there is no Plan until the next
  hand-off (the owner's call of 2026-11-10 on the plan bar's stale round, kept here).
- **The team is one line each:** initial, name, what the agent is doing in one phrase, and its tasks in
  this chat as done/total. No role, no start time, no chevron. Who needs the person comes first, then who
  works, who waits, who is ready.
- **An agent opens as a workspace tab,** beside Tasks, from a step or a line in Summary or from its initial
  in the timeline: its role and state with when it began, Message, the task it is on, its running turn's
  row opened, its earlier blocks in this chat (the latest 10) and its tasks. One agent at a time. The row
  and the blocks are the timeline's own components.
- **The Working line opens Summary** too, so "who is working" leads to what they are doing.
- **Pause or resume for one agent** is a menu at Summary's foot. An agent's context bar is in its panel.

## Why

- The plan bar and a status bar each said what Summary already says: who is working, on what, and how far
  along the round is.
- The turning rings already show who is working, so words on the button would say it twice.
- Rows are one line so the team reads at a glance. Start times moved to the agent's panel, where there is
  room for them.
- A panel beside the chat keeps the timeline in view while one agent's work is read closely.

## Turned down

- **Keeping the plan bar.** It repeats Summary.
- **Words on the button** ("2 working · 1 needs you"). The rings and the dot already say it; the words stay
  as the button's accessible name and tooltip, and beside the Team heading.
- **A mark on one agent's initial for "needs you",** instead of one dot on the stack. The owner chose one dot
  for the stack, the same dot the sidebar's row is to carry.

## What this slice leaves

The registry's AgentPlan and SubagentList files are removed, as no screen uses them; TodoList stays, in the
agent's panel. The conversation map, thinking, tool arguments and the sidebar's dots are later slices.
