# 0037. The chat is one timeline

Status: accepted (owner, 2026-11-11: "Build it", on the chat redesign's approved design). Date: 2026-11-11.
Replaces 0015's split chat and the two columns 0028's order was drawn in. Summary, the plan bar and the
recipient rules of 0015 and 0028 stand until their own slices of the redesign.

## Choice

- **One chat in order,** for one agent or five. No Supervisor column and Team column, no header toggle,
  nothing kept per chat about a split.
- **A turn is one block, where it began.** Between turns the service's `position` decides (0028), so
  replies streaming at once never interleave, and a later reply of a turn lands in that turn's block.
  Only a person's message sent into a running turn splits it: one block before the message, one after.
- **What a turn did folds into one row above its message.** Finished, the row reads
  "Worked for 1m 49s · 6 steps · gave 2 tasks", with a part left out when it is zero. Running, it says
  what the agent is doing now and shimmers, with no spinner, then its steps and how long it has run.
  Opened, it lists each tool call as a step in plain words (its time at the right, a failed one marked)
  and what the agent wrote between steps as quiet lines. A task it gave reads "Gave Cass a task" with the
  task's number and title, and opens the task. The turn's last text is the message, at full strength. A
  turn that only wrote once has no row.
- **Who speaks.** The lead reads at 16px; a member's or reviewer's turn at 15px, headed by its name, the
  task it works on ("on Open the settings tools PR") and the time. A speaker is named when it changes, not
  on each of its blocks in a row.
- **Agents' messages to each other are their own block,** tinted, each headed "Juno to Orin" with its
  time. Messages that follow each other share one block, and an answer sits beside the message it
  answers. Gathr's copy of a message in the receiver's turn stays hidden.
- **What Gathr tells an agent is a one-line event,** with no "Gathr" speaker. A hear-back draws a line
  for each task it reports, in the state that hear-back gave ("Vale completed: Review PR #138", "Cass is
  blocked: Fix the build"); the person's answer reads "You answered Orin: Hold it"; a question to the
  person "Orin asked you: Ship it?"; anything else (a wait's result, a wake-up, a nudge) is drawn in
  Gathr's own words. A line opens in place to the reason or the question, and to its task. Two things
  are never drawn: the brief of a member's own task, because the member's block is headed by the task
  and the giver's row lists it, and Gathr's copy of an agent's message.
- **Approvals** keep their card where the agent asked, and also dock above the composer: the oldest one
  waiting, how many wait, and Show, Deny and Allow.
- **"Working"** stays after the latest message while any turn runs: who is at work, and for how long.

## Why

- The team column read like task lists. The work was there, but the talk was lost in it: messages
  agents wrote to each other were under 1% of what the column drew.
- Measured in one real chat of 4,537 messages: 2,889 were tool calls, 1,288 agent texts, and 34 were
  messages addressed from one agent to another. Folding a turn's tool calls and mid-work text into one row
  leaves the words that matter.
- In 424 real turns, 307 wrote more than once and none ended with a tool call after its last text. So
  "the last text is the message" holds, and mid-work text is common enough to need its own quiet place.
- Two columns asked the person to keep two places in one conversation, and to guess which column a reply
  would land in. One order needs no toggle, no unseen dot and no kept reading position.

## Turned down

- **Keeping two columns.** The split is what hid agent-to-agent talk and made hand-offs read as lists.
- **A team column folded by default.** It hides the team's work behind a click and keeps the second
  place to look.
- **Opening an agent beside the chat as the only way to see its work.** The agent panel comes in a later
  slice as a closer look, not as the only one: the timeline has to stand alone.

## What this slice leaves

Steps show no thinking until the service keeps it; what a step ran is kept since
[0041](0041-what-a-step-ran.md). Summary, the plan bar, the agent
panel, the conversation map and the sidebar's dots are later slices of the same redesign.
