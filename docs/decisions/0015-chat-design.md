# 0015: Desktop chat with split team conversation and Summary

Status: accepted

## Choice

Use a sidebar of projects and their chats, one centred conversation, a rounded composer and a side panel.
The same conversation and composer serve one-agent chats. DESIGN.md and designs/system.html record the selected system.
The polished pages are listed in designs/README.md.

### Accepted split chat refinement, 2026-11-10

The owner's split_chat/README.md and Split Chat.dc.html replace the inline thread
below. This records the target before implementation, not completed UI work.

- A header toggle opens one chronological team chat beside the Supervisor's
  conversation, 50/50. Each scrolls independently. The composer stays on the left;
  the workspace still opens to the right of both columns.
- Main keeps the person's messages, Supervisor hand-offs/final words and direct
  @member exchanges. Hand-off replies move to the team column, with their
  assignment lines, ordered by turn start (0028). Concurrent turns never interleave.
  No Team thread disclosure or View team thread link remains.
- Persist the split per chat. Keep draft, recipient and both reading positions.
  A closed split gets a new-message dot; opening clears its header dot. Late work
  updates the same chronological team chat without moving the reader.
- Waiting on team opens the split; Working ends the team column. Their initials
  come from running turns; the Supervisor's named wait supplies its start time.
- An agent-plan bar docks above the composer. Its checklist is the round's real
  tasks (given_in links them to one Supervisor turn). Until plans are records,
  its title is Plan. Never derive a plan title or step from message prose.
- Summary remains a popover: official SubagentList rows, progress and expandable
  TodoList assignments. Counts are done/total per agent in this chat, excluding
  cancelled tasks. States and wait starts come from tasks, turns and waits.
- The thread and composer maximum becomes 52rem. Agent bodies indent 34px to
  align with names. Archived uses an open-lid Archive icon when expanded.
- Reduced motion stops spinners and removes the chevron and lid transitions.

Why: one running team conversation keeps parallel work readable without reopening
each hand-off. The Supervisor remains the primary conversation, and Summary stays
available for a quick scan. This replaces the earlier rejection of a team pane;
it does not introduce separate panes per agent or consume the workspace.

Turned down: retaining inline threads, hand-off-specific panes, and porting the
prototype HTML/runtime into ui/. Use existing assistant-ui registry components
and Gathr state. DESIGN.md and system.html carry the component anatomy; the
service remains the authority on assignment and completion (0010).

### Superseded inline refinement, 2026-11-08

For team chats, put the members' work in an inline thread under the Supervisor's hand-off.
Summary, the prototype's floating popover in the chat header, replaces the earlier Team tab.
It shows each agent's role, state, named wait and assignments without taking over the workspace.

- Group by the message that gave the task, never by the preceding message or arrival time.
- The open thread grows in the conversation, with no inner scroll box. Keep turn-start order inside it (0028).
- Open while running and fold on completion, unless the person has set it open or closed. Retain their choice.
- A late reply updates its original thread. The Supervisor's next word arrives at the bottom of the conversation,
  after anything said in between, with a link that opens the thread and focuses its disclosure.
- A status message nobody answers has no thread. Direct @member exchanges stay in the main conversation.
- A reviewer's agreement is a named reaction on the final word; a question stays a reply.
- Incoming work never moves the reader or clears the draft. Summary and thread disclosure preserve both.

This selects the presentation; it does not claim grouping exists in the service.
F5 must record a task's originating turn and a turn's task before F8b can group replies.
The service remains the authority on assignment and completion (0010).

## Why

The conversation stays readable while coordination remains available beside it. New and Search share outline icons.
User messages fit their text; agent replies remain unboxed. Both themes use neutral surfaces and one restrained accent.

The superseded inline choice kept work beside the message that asked for it. The
accepted split instead gives team work its own scroll area while keeping Summary
available for a quick scan of assignments and progress.

## Alternatives

A live agent strip crowded the composer. Pinned working turns reduced conversation space. Earlier overview and room directions were rejected in favour of one desktop chat shell. The Team-panel direction was selected as v10 and subsequently refined without changing its layout. Earlier directions remain in git history.

The superseded inline-thread refinement replaced that Team-panel choice. It rejected: every reply expanded in the main line
(long replies bury the conclusion), member tabs (hide parallel work), a side-panel thread (competes with Tasks and Files),
and simultaneous agent panes (too little reading height at five agents). The side panel and agent panes were compared
with inline threads at 1440 and 1024px, light and dark, with running, finished and history fixtures.

### Accepted archive refinement, 2026-11-09

Put archived chats in a folded Archived group after active chats, with a count.
This follows the sidebar's existing folds and adds no destination. Restore
returns the chat to its original project without restarting its cancelled work.
Turned down: an Archived button above Settings opening a separate navigation
flyout. Its independently scrolling history was useful for long lists but added
a destination and another transient surface. The folded group is the selected
layout from archive-stale/v1; v3 records its confirmed copy and day-count range.
DESIGN.md and system.html carry the shared presentation; 0010 owns the effects.
