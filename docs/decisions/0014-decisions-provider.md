# 0014. Decisions: fast yes/no answers, optional from day one

Status: accepted. Date: 2026-11-07.

Built: no. Nothing of it is in the code.

Out of date below: "from day one" in the title and "from the first version" below.

## Implementation status

Not built yet. The accepted provider, interface, stand-in, Settings page and uses below remain planned; the first slice did not include them.

## Choice

Gathr has **Decisions** from the first version: a small, fast model that answers narrow questions about some text (yes or no, one of a few choices, or a score) with a probability, in about 150 ms. The service and agents use it where asking a full engine would be slow or wasteful.

It's **optional and off by default**. It runs on a paid API, so it needs the person's own key, and nothing in Gathr may need it to work.

- One `Decider` interface in the service: `ask(use, questions) -> answers`, each answer with its probabilities. The first provider is TypeSafe's Jev (`POST /v1/systemone` with a bearer key; versions from `GET /v1/models`, pinned, with the pin a setting).
- Every use has its own switch and its own threshold, set in code per use: from about 0.65 where an answer only adds a hint, up to 0.95 where it lets something through. A probability under the threshold counts as no answer.
- **No answer is always safe.** When Decisions is off, has no key, has that use switched off, is over the cap, is paused, times out or errors, the feature does what it does without Decisions. Nothing waits more than 1.5 s when the person is waiting, or 10 s in the background.
- A 402 or 429 from the provider pauses Decisions for 24 hours. A long batch slows to one request at a time on a 429 and retries up to 4 times first.
- **Spend cap:** $1 a day by default, settable from $0.01 to $100. Each call's estimated cost is reserved against the cap before sending and replaced by the real token count after. At the cap, Decisions is off until midnight, with one notice. A batch estimated at $0.25 or more, or more than is left today, asks the person first.
- **Redaction:** the same secret redactor as raw engine frames ([0011](0011-event-record.md#raw-engine-frames)) runs on every question before it leaves the machine. Login files, keys and tokens never do.
- **Settings, Decisions:** on or off, the key (stored as a secret, shown masked), the version pin, the cap and today's spend, each use's switch, and a Test button. The Decide tool also has a per-agent switch on the agent's page.

## Uses

| Use | Asks | Arrives with |
|---|---|---|
| Decide tool | An agent asks narrow questions about text, or each row of a sheet | The MCP server's tools |
| Memory saves | Which sentences of the person's message are worth remembering that memory missed | Memory |
| Watch long work | Is an agent going in circles, stuck or off track, or does it need the person (a look every 30 s on long work, 10 s budget) | Chats with several agents |
| Skill suggestions | Did a finished chat likely teach a skill worth writing; only then is the full skill review run | Skills |
| Memory tidy | Which lines repeat or are replaced (nightly) | Memory |
| Approvals | Whether a request is safe to let through without asking (threshold 0.95) | Approvals; off by default |

The previous app's uses for watchers, reminders, the job board, channels, workflows, goals and rooms are left out with those features. "Watch long work" was called the supervisor there; it's renamed so it isn't confused with a chat's Supervisor ([0013](0013-one-chat-model.md)).

A future feature builds the `Decider` interface, local stand-in provider, Settings page and cap. The originally planned PR immediately after the first slice did not happen. Each use still arrives with its feature.

## Events

Each call is a `gathr.decision` span in the event record ([0011](0011-event-record.md)): the use, the number of questions, the provider and version, input tokens, cost, latency, and the outcome (`answered`, `below_threshold`, `off`, `capped`, `paused`, `timeout`, `error`). Never the question text or the key. The analytics page shows calls, spend and outcomes per use, so the person can see which uses earn their cost.

## Why

- The previous app used it 1,104 times in 20 days for $0.25, with a call taking 120 to 160 ms. That's the speed and price that makes per-message and per-30-second checks possible; an engine turn for each would cost minutes and real usage.
- Treating "no answer" as "behave as if it were off" kept the previous app working through outages, caps and missing keys without special cases.
- The previous app's client and stand-in server are the owner's own code (added after the Bloks import), so they're ported, not rewritten.

## Turned down

- **Required, or on by default.** It's a paid API; Gathr's rule is that anything that costs money is opt-in and never required.
- **Using an engine (Claude Code or Codex) for these questions.** Seconds per answer and real usage from the person's subscription, for questions that need a probability, not a conversation.
- **Hard-coding one provider.** The interface keeps a second provider, or a local model, one driver away.

## Not verified

- The request and response shapes come from the previous app's client and its stand-in server, not from TypeSafe's own documentation. The Decisions PR checks them against the docs before the real provider ships.
- The price per million input tokens is set in the previous app's code; it's checked against the provider's current price before the cap's estimates are trusted.

## Code to port

From the previous app, all the owner's own: the client (`judge.ts`), the Decide tool (`decide-tool.ts`), the watcher (`supervisor.ts`), the checker (`checker.ts`), and the stand-in provider for tests (`fake-jev.ts`), which comes first so tests run with no key and no network.
