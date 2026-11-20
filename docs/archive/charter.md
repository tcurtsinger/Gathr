# Gathr's charter

What Gathr is for, how work moves through it, and what it promises. Every plan, decision and task
serves something on this page. It changes only when the project's owner says so.

Three words mean the same thing here, in the decisions and in every plan:

- **Written:** a document says it.
- **Built:** code does it, and a named test fails if it stops.
- **Kept:** built, and its number has held in real use for three days.

Only Kept counts as finished. Most of this page is Written today. Each decision says what is Built.

Two names. Your **assistant** is the agent you talk to when you name nobody. A project's **lead** runs
its team. The code and the older decisions call the lead the Supervisor, and have no assistant yet.

## What Gathr is for

Gathr is one place to work with your own agents on anything: an everyday question, a document, a
software project. It works like an organization. You talk to your assistant. The assistant does small
things itself. When the work is a project, the assistant brings in a team of specialists with a lead,
you plan it with the lead, and the team builds it, checks it and lands it. You always know where it
stands, and you never have to write a careful prompt or a rulebook to make that happen.

How it goes:

1. You open Gathr for the first time, set your engine, and create an agent or several. You choose your
   assistant.
2. You start a new chat. Type @ and an agent's name, and you are talking to that agent. Name nobody, and
   you are talking to your assistant. There is no agent button to find first.
3. You ask for help with something everyday. The assistant does it with the tools it has, or hands it
   to the specialist it belongs to.
4. You say you want to start a project. The assistant suggests a team from your agents: up to five in
   all, one of them the lead and one a reviewer. The team is agreed before any planning starts.
5. You plan it with the lead. Then the work follows the loop below.

**Staff and seats** are two different things.

- **Your staff** are your standing agents: a fantasy sports coach, a help desk agent, a frontend
  engineer. Each has a name, a job description, its own engine and its own know-how, which grows. You
  create them, or the assistant proposes one and you say yes. You talk to any of them by @-ing them.
- **A seat** is a job on one project: lead, reviewer, builder. A project's team is drawn from your
  staff. You can set it by hand, or agree the one the assistant proposes.
- **Your assistant** is one of your staff with one more job: it answers when you name nobody, knows who
  on the staff does what, hands an ask to the specialist it belongs to, and sets up projects. It can
  lead a project too.

What an agent knows lives in three places. About you: shared by every agent. About a project: with the
project, for whoever is on its team. About a craft: with the agent, wherever it goes.

**It reads like a company's chat.** You @ an agent to talk to it. Agents @ each other to talk to each
other, in the open, in the same chat, so a team's work reads like a thread. An agent that only agrees
reacts to the message with a thumbs-up; it never posts a message to say so. In navigation a chat sits
under its agent's name, inside its project when it has one. There is no general chat: a chat exists
because you started one.

**Open both ways.** An outside AI app can bring something to your team through Gathr and hear back, as
a named guest in the chat. Your agents can reach tools and services outside Gathr. Both go through MCP,
and both are yours to allow.

The engines (Claude Code, Codex and later ones) do the thinking. Gathr holds everything between them:
who does what, in which order, what is finished, what is waiting, and what you must decide.

**Who it's for:** one person running their own agents on their own computer, every day. Making it easy
for other people to install comes after the promises below are Kept.

Two runs test the whole product:

> **The everyday run.** In a new chat, with no agent named, the person asks for something small. The
> assistant answers and does it with its tools. No team, no plan card, no questions it could have
> answered itself.

> **The project run.** In a new project, with Gathr's defaults untouched and no hand-written rulebook,
> the person types "build an export for my notes". The assistant proposes a team and the person agrees
> it. The lead asks what it must, writes a plan, the reviewer checks it against the code, the person
> approves it, the parts go out with complete briefs, the work is built in each agent's own place,
> reviewed, landed and shown working with proof. The person never had to ask what the room was waiting
> on.

## The loop

**First, the ask is sized** by whoever you are talking to: your assistant in a new chat, the lead in a
project. The size of the process fits the size of the ask. That is how Gathr balances quality and
speed.

| Size | What it is | What happens | Asked first? |
|---|---|---|---|
| Everyday | A question, a lookup, a document, a small job | The assistant does it, or hands it to a specialist | No |
| Repair | Something that worked is broken, or a defect | A short brief, built, reviewed, shown to the person after | No |
| New | A new feature, a project, a project plan, anything that could change the direction | The full loop below | Yes |

When it can't tell a repair from something new, it is new. The person can say "this should have been
asked", and Gathr counts that as a correction.

**The full loop,** for new work. Each step is a state Gathr holds, with a gate, never something read
from an agent's words ([0010](decisions/0010-coordination-as-state.md)).

| # | Step | Who | What Gathr holds |
|---|---|---|---|
| 1 | Ask | the person | the message |
| 2 | Size | your assistant, or the project's lead | everyday, repair or new |
| 3 | Team | the assistant proposes, the person agrees | the team: who leads, who reviews, who builds, on which engine |
| 4 | Plan | the lead, with the person | the lead's questions and the answers; then a plan in parts, each with an owner, what done means, how it's proved, its order |
| 5 | Check | the reviewer | the check's result: agree, or objections with evidence |
| 6 | Approve | the person | approved, sent back with notes, or dropped |
| 7 | Hand out | Gathr, from the approved plan | one task per part, each with its complete brief |
| 8 | Build | members | each agent's report: done, left or blocked, with what to look at |
| 9 | Review | someone who didn't make it | approved at an exact version |
| 10 | Land | Gathr | the work is where it belongs: merged, or saved |
| 11 | Prove | the lead | the proof, shown to the person |
| 12 | Learn | Gathr | friction and the scorecard, feeding the next plan |

At step 4 the lead asks the questions whose answers change the work, one at a time, with options and its
pick, until the direction is exact. It doesn't ask what it can find in the code or the docs itself.

For work that isn't code, review means a check by someone who didn't write it, and landing means the
result saved where it belongs. A repair runs steps 7 to 11 on a short brief.

## The promises

Each promise is something a test can prove. Everything Gathr builds serves one of them.

1. **An assistant by default, your own specialists, and a team when the work needs one.** A new chat
   with no agent named is with your assistant; @ an agent and you talk to it. The assistant does small
   things itself and hands an ask to the specialist it belongs to. For a project it proposes a team from
   your agents, with a lead, and the team is agreed before planning starts.
2. **New work is planned with you and checked first. Repairs don't wait for you.** Nothing new is built
   before you approve its plan, and no plan reaches you before the reviewer has checked it against the
   code. Fixing what's broken doesn't ask.
3. **Every task stands on its own.** A task carries what to do, why, what done means, how it's proved
   and where the work lives. An agent with no memory of the chat can do it right.
4. **One voice to you.** On a team, only the lead asks you, in its own words, with its pick first. An
   answer reaches everyone who waits on it at once.
5. **Work tells the truth.** A task's state is what its owner reported plus what Gathr can see. "Done"
   says what to look at. A turn ending is never read as work ending.
6. **The room explains itself.** At any moment Gathr can say who is doing what and what the room is
   waiting on. A quiet room is noticed by Gathr, before you notice. Agents talk to each other in the
   open, by @-ing each other in the chat, so a team's work reads like a thread. Agreeing is a reaction,
   never a message.
7. **Everything in its place.** What an agent makes stays in its own folders. What it starts ends with
   its work.
8. **Context lives in the record.** An agent picks up any task after a compaction, a restart or a new
   session and loses nothing, because the task and the plan carry it, not the conversation.
9. **Any engine, same behaviour.** Claude Code, Codex and later engines behave alike in a team. Mixing
   them is a strength: a review by a different provider.
10. **Quality and speed in balance.** Proof and review are never skipped to go faster, and nobody waits
    on nothing. The size of the process fits the size of the ask.
11. **No rulebook needed, and Gathr measures it.** A new project works on defaults. Gathr counts how
    often you had to correct the room, and shows it.
12. **Open both ways.** An outside AI you allow can bring something to your team and hear back, as a
    named guest in the chat. Your agents can reach outside tools. Nothing from outside speaks as you.

## How this page is used

- A plan names the promise it serves. A plan that serves none isn't built.
- A decision in [`decisions/`](decisions/) says how a promise is kept, and whether that is Written,
  Built or Kept.
- A rule in [`AGENTS.md`](../AGENTS.md) that makes up for something Gathr doesn't do yet names the
  promise it stands in for, and goes when that promise is Built.
