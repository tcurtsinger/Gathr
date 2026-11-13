# 0013. One chat model

Status: accepted (the owner's call). Date: 2026-11-07.

Built: partly. Proof: `crates/gathr/tests/team.rs`, `crates/gathr/tests/chats.rs`. Not built: a review task that starts by itself ([0043](0043-checks-and-merge.md), C9); plan checks; the thumbs-up reaction; the global shortcut for a new chat.

Out of date below: adding a second agent makes the first one the Supervisor by itself, without asking. [The charter](../charter.md) names the two jobs assistant and lead: this file's Supervisor is the lead, and no assistant exists yet.

## Choice

Gathr has one place to talk to agents: a **chat**. There are no separate rooms, no separate one-agent chats and no Quick ask window with its own model. Each of those is a chat.

- A chat has **1 to 5 agents**. The service refuses a sixth.
- With **one agent**, every message goes to it.
- With **two or more**, the chat has a **Supervisor**, required, and a **Reviewer**, optional. Adding a second agent asks for the Supervisor first. A message that names nobody goes to the Supervisor; `@Name` passes the turn to that agent.
- A chat may have a **project folder**, set when it's created. Then it's that project's chat: its agents work in that folder and read the project's `AGENTS.md`. Without a folder it's a plain chat.
- The global shortcut opens a new chat; it isn't a different kind of thing.

## Roles

| Role | Who | Does |
|---|---|---|
| Supervisor | one of the chat's agents; required with 2 or more | Gets unnamed messages, hands out tasks, has the last word, answers plan checks' objections. Replaces the previous app's room lead. |
| Reviewer | one of the chat's agents, optional, not the Supervisor | A review task is to start for it by itself when a member's task ends with changes or a pull request opens; that isn't built, and today the Supervisor gives the review ([0043](0043-checks-and-merge.md), C9). Checks the Supervisor's plans first, once plans are built ([0010](0010-coordination-as-state.md#plans-and-plan-checks)). |
| Member | the other agents | Take tasks, ask teammates, check plans when there's no Reviewer. |

Changing the Supervisor or Reviewer is a setting on the chat, recorded as an event ([0011](0011-event-record.md)). A Supervisor can't be removed from the chat while it's the Supervisor; pick another first.

## What follows

- Coordination scopes are the app, a chat and an agent ([0010](0010-coordination-as-state.md)). Where 0010 says "chat", a one-agent chat simply has nobody to hand off to.
- An agent can be in many chats. It runs one turn at a time across all of them ([0010, Sessions](0010-coordination-as-state.md#sessions)).
- An agent may react to a message with a thumbs-up through a tool instead of posting a message that only agrees. A reaction is an event, not a turn for anyone else. Not built yet.
- "One main agent" is a chat whose Supervisor is the agent the person talks to most; it needs nothing extra in this model.

## Why

- The previous app had chats and rooms as two models, and the same tasks showed four different ways across them. One model means one way to show tasks, side jobs, waits and pause.
- The difference between a chat and a room was only how many agents were in it. Making that a number, not a type, removes a whole set of screens and rules.
- A required Supervisor gives every multi-agent chat a clear owner of unnamed messages and the last word, which the previous app's rooms needed and had.
- Five agents is the size the previous app's working rooms reached; past that, nobody can follow the chat.

## Turned down

- **Rooms and chats as separate features.** Two models for one idea, and every screen built twice.
- **No limit on agents.** A chat of a dozen agents can't be followed, and the person's usage runs out faster than the work moves.
- **Supervisor optional with several agents.** Then nobody owns an unnamed message, and the chat stalls or everyone answers.
