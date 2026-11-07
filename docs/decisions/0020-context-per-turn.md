# 0020. Context per turn: lean by default, measured, never trimmed blind

Status: accepted (the owner's requirement). Date: 2026-11-07.

## Choice

Each turn gives an agent what it needs to do excellent work and nothing it can fetch when it needs it. What Gathr adds is small, stable and measured, and any change to it is judged on output quality as well as tokens.

### What Gathr puts in a turn

| Part | What | Size |
|---|---|---|
| Briefing | Who the agent is, its role in this chat, who else is here and what each knows, the project's `AGENTS.md` by reference, how to ask, wait and hand off | Stable, the same every turn of a session, so the provider's prompt cache keeps it |
| The ask | The message or task, its files, its `after` results | Whole |
| Standing rules | What still applies however old it is: the approved plan the task belongs to (its part, with the plan's id), the person's decisions that bind this chat or project, and the project's kept notes | Short, each with its source id; never dropped from a fresh session because it's old |
| Catch-up | For a fresh session: what changed in the chat since the agent last worked here, and the agent's own recent work in this chat (what it did, decided and left open) | A digest, with ids to fetch the rest |
| State | Its open tasks, waits and side jobs, as one short block | Whole |

Everything else is pulled, not pushed: Gathr's MCP tools read the chat's history, a task, a plan, a file card, a diagnostics trace or memory when the agent asks for it. The engines' own tools read code.

### How it stays lean

- **A fresh session per task** when the agent already has other work in progress ([0010](0010-coordination-as-state.md#sessions)), with the catch-up above; the earlier session is set aside whole, not summarised away.
- **Compaction is the engine's,** plus the person's own Summarise now and Summarise at (kept from the previous app at the owner's request). Gathr never compacts behind the person's back.
- **Gathr's tools return what was asked,** sized for a context: a long result comes back as a summary with an id to fetch the whole of it. Built-in engine tools are the engine's to manage.
- **Reviews and plan checks run in a fresh session** with the plan, the diff and the files around it, not the builder's history.
- **Stable first:** the briefing and the ask's fixed parts come first and don't change mid-session, so cached input (`gen_ai.usage.cache_read.input_tokens`) stays high.

### How it's measured

The engines report totals per turn, not parts: input, cached reads and writes, output ([0001](0001-engine-interface.md), [0011](0011-event-record.md)). Counting tokens per part would need a provider API key, which [0002](0002-login-folders.md) rules out. So every turn records:
- the provider's totals, exactly as reported;
- the size of each part Gathr adds (briefing, catch-up, state, the ask) in characters, with a local token estimate labelled as an estimate;
- tool results by kind and full size ([0011](0011-event-record.md)).

The engine's own share is the remainder of the total after Gathr's parts: an inference, shown as one. The analytics page shows all of it per agent, chat and engine, with the turns that compacted and what each cost.

### Quality is the other half

Context is never cut on token counts alone. A change to what Gathr puts in a turn (a smaller catch-up, a shorter briefing) is compared before and after on the outcome signals the record already has: turns that failed or were redone, review findings per PR, tasks reopened or blocked, side jobs asked because something was missing, and the person correcting an agent. A change that saves tokens and costs quality is undone.

## Why

- The owner's requirement: agents shouldn't carry bloated context each turn, and their output must stay excellent.
- Lessons from the previous app: rooms compacted about ten times a night at around 467K tokens; members carried 200K tokens a request while every task shared one session; and an agent in a fresh session didn't know its own work from minutes before, so lean context without the agent's own recent work cost quality.
- Pulled context is exact and current; pushed context goes stale and costs every turn whether it's used or not.

## Turned down

- **The whole chat history every turn.** What the previous app's rooms did; the cost grows with every message and compaction loses detail unpredictably.
- **A fixed token budget that trims to fit.** It drops whatever is last, not whatever is least useful.
- **Gathr summarising sessions itself.** The engines compact better than a layer on top can, and the person already has Summarise.

## Not verified

The sizes Gathr's parts end up at, and which outcome signals move when they change, come from the first months of the record. Also whether the local token estimate of Gathr's parts is close enough to judge a change by: the first slice compares it with the provider's totals on turns where Gathr's parts are most of the input. The first slice records, for each turn on one chat, the provider's totals and the characters of the briefing and a fresh session's catch-up, so the split above is checked on real turns. It also checks that a fresh session's turn carries the project's kept notes. When plans and tasks land (0010), a test at the service API checks that a fresh session for a task carries its approved plan and a binding decision made long before the digest's window.
