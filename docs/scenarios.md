# Scenarios

Each promise in [the charter](charter.md#the-promises) as scenarios in plain steps: given, when, then.
A scenario is what a test will prove. They are written here before the code.

How they are used:

- When work starts on a promise, its scenarios become tests first, and the tests fail first. They run the
  real service with the replay engine, once for each engine.
- A scenario names the parts of [the truth map](truth-map.md) it proves. When a part is built, its row
  there names the test. Most of these scenarios can't pass today: the truth map says what exists.
- A scenario says what Gathr holds, does, shows or refuses. It never rests on how well a model writes,
  so a replayed engine can prove it. What only a real engine can show is listed
  [at the end](#what-a-replay-cant-prove).
- `node scripts/docs.check.mjs` fails when a scenario names a part the truth map doesn't have, or a
  part has no scenario.

The people in them: **the person**; their **assistant**; on a team, the **lead**, the **reviewer** and
a **builder**. "Either engine" means the scenario gives the same result with Claude Code and with Codex
in that seat.

## 1. An assistant by default, your own specialists, and a team when the work needs one

### 1a. A new chat with nobody named is with the assistant

Parts: 1.1, 1.9

- **Given** the person has several agents and has chosen one as their assistant
- **When** they start a new chat and send a message that names no agent
- **Then** the assistant gets the message and answers, no agent had to be picked first, and the chat is
  listed under the assistant's name

### 1b. @ an agent in a new chat, and you are talking to it

Parts: 1.2, 1.9

- **Given** the person has an assistant and one other agent
- **When** they start a new chat and their message begins with @ and the other agent's name
- **Then** that agent gets the message, the assistant doesn't, and the chat is listed under that
  agent's name

### 1c. In a team chat, @Name reaches that member

Parts: 1.3

- **Given** a team chat with a lead and a builder, open in the window
- **When** the person sends one message that @-names the builder, and one that names nobody
- **Then** the builder gets the first and the lead gets the second, whichever agent the window showed
  last

### 1d. An agent is created with what it is for

Parts: 1.4, 1.5

- **Given** an account signed in on an engine
- **When** the person creates an agent with a name, that account, a model, an effort and a job
  description
- **Then** the agent is saved with all five, its turns run on that account, model and effort, and
  every agent that is told about it (a teammate, the assistant) is told its job description

### 1e. The assistant hands an ask to the specialist it belongs to

Parts: 1.6

- **Given** an assistant, and a specialist whose job description covers the ask
- **When** the person asks the assistant, and the assistant hands the ask on
- **Then** the specialist gets the person's ask as it was written, the hand-over shows in the chat, and
  the specialist's answer reaches the person in that chat

### 1f. A project's team is proposed and agreed before any planning

Parts: 1.7, 1.8

- **Given** the person has more than five agents
- **When** they tell the assistant they want to start a project
- **Then** the assistant proposes a team from those agents: five at most, one lead, one reviewer. No
  plan is started, and no task given, until the person agrees the team or changes it

### 1g. A team set by hand holds the same limits

Parts: 1.8

- **Given** a new project
- **When** the person sets its team by hand with six agents, or with two leads, or with no reviewer
- **Then** each is refused, saying which limit it breaks, and a team of a lead, a reviewer and a
  builder is accepted

### 1h. What an agent knows lives in three places

Parts: 1.10

- **Given** an agent has kept one note about the person, one about a project, and one about its craft
- **When** another agent starts a chat, a new member joins that project, and the first agent starts
  work in a different project
- **Then** the other agent has the note about the person, the new member has the project's note, and
  the first agent has its craft note in the new project and not the old project's note

### 1i. A chat sits under its agent, inside its project

Parts: 1.9

- **Given** a project with chats with two of its agents, and one chat that belongs to no project
- **When** the person looks at navigation
- **Then** the project shows each agent's name with that agent's chats under it, and the chat with
  no project sits under its agent's name outside every project

## 2. New work is planned with you and checked first. Repairs don't wait for you

### 2a. Every ask is sized, and Gathr holds the size

Parts: 2.4, 10.3

- **Given** a project with a lead
- **When** the person asks a question, reports something broken, and asks for a new feature
- **Then** each ask has a size on record: everyday, repair, new. The question is answered with no task
  and no plan. The repair gets a short brief. The feature starts a plan. An ask the lead leaves
  unsized is treated as new

### 2b. Nothing new is handed out before its plan is approved

Parts: 2.2

- **Given** an ask sized as new, and its plan not yet approved
- **When** any agent, the lead included, gives a task for it
- **Then** Gathr refuses and says the plan isn't approved. The same task is accepted once the person
  has approved the plan

### 2c. No plan reaches the person unchecked

Parts: 2.3

- **Given** the lead has written a plan
- **When** the lead puts it to the person
- **Then** Gathr gives it to the reviewer first. The person sees the plan only with the check's result
  on it: agreed, or objections with their evidence and the lead's answer to each

### 2d. A plan is a record with versions and states

Parts: 2.1

- **Given** a plan the person sent back with notes
- **When** the lead revises it
- **Then** Gathr holds both versions and the notes. The new version is checked again before the person
  sees it. At every moment the plan is in one state: being written, being checked, with the person,
  approved, sent back, or dropped

### 2e. The lead asks its questions first, one at a time

Parts: 2.5

- **Given** an ask sized as new, where two things must be decided before a plan can be written
- **When** the lead asks the person
- **Then** the first question arrives alone, with options and the lead's pick. The second arrives after
  the first is answered. Both, with their answers, are held with the plan, and the plan can't go to
  its check while a question is open

### 2f. A repair doesn't wait for the person, and is shown after

Parts: 2.6

- **Given** an ask sized as a repair
- **When** the lead gives its short brief
- **Then** the task is handed out with no plan and no approval, and is built and reviewed. When it
  lands, the person is shown what was fixed and its proof, and can mark it "this should have been
  asked"

## 3. Every task stands on its own

### 3a. A task carries a complete brief, or it isn't given

Parts: 3.1, 3.2, 3.3, 3.4

- **Given** an approved plan, or a repair
- **When** a task is given
- **Then** it carries what to do, why, what done means, how it's proved, and where the work lives: the
  project, the folder and the branch. A task that lacks one of these is refused, naming what is
  missing

### 3b. Approving a plan makes its tasks

Parts: 3.5

- **Given** a checked plan in three parts, each with an owner, what done means, how it's proved and
  its order
- **When** the person approves it
- **Then** Gathr makes three tasks, each with its part's brief, owner and order. Nobody types them
  again

### 3c. A task set after another starts when that one's work is done

Parts: 3.6

- **Given** two tasks, the second set to come after the first
- **When** the first is reported done and waits only on its merge
- **Then** the second starts. While the first is still being worked on, or was left or blocked, the
  second doesn't start

### 3d. An agent that never saw the chat can do the task

Parts: 3.7, 8.8

- **Given** a task with a complete brief, and an owner with no session in that chat
- **When** the task starts in a new session
- **Then** the first thing the agent is given holds the whole brief: what, why, done, proof and where.
  It works in the folder and on the branch the brief names, and nothing it needs is only in the chat

### 3e. A task set after one that stopped starts when that one finishes

Parts: 3.6

- **Given** a task set after another, which was left, failed or stopped
- **When** that other task is restarted and finishes
- **Then** the task set after it starts, and so does the next one down the chain

## 4. One voice to you

### 4a. A member's question goes through the lead, in the lead's words

Parts: 4.1, 4.3

- **Given** a team with a lead and a builder
- **When** the builder needs an answer from the person
- **Then** the builder's ask becomes a request the lead owns, and the builder never asks the person
  directly. The lead answers it when it can. Otherwise the question that reaches the person is
  text the lead wrote: Gathr has no way to forward the builder's text as it is

### 4b. A question is a record, with the lead's pick first

Parts: 4.2

- **Given** the lead asks a question with three options and names its pick
- **When** the person opens it
- **Then** the pick is first and marked as the lead's pick. The person can choose an option, or answer
  in their own words without choosing one. The question, its options, the pick and the answer are
  held

### 4c. An answer reaches everyone who waits on it, at once

Parts: 4.4, 4.5

- **Given** a builder's task waits on a question the lead put to the person
- **When** the person answers
- **Then** the builder gets the answer and carries on with no turn of the lead's in between, and the
  lead is told too. Until the answer comes, the builder's task stays open and says it waits on the
  question

## 5. Work tells the truth

### 5a. The owner reports a task: done, left or blocked

Parts: 5.1, 5.3

- **Given** a builder working on a task
- **When** it reports
- **Then** "done" says what to look at: a PR, a commit, a file or a note. "Left" says what is left.
  "Blocked" says on what. A "done" with nothing to look at is refused, saying what is needed

### 5b. A turn that ends is not a task that is done

Parts: 5.2

- **Given** a builder working on a task
- **When** its turn ends and it has reported nothing
- **Then** the task is not marked done, and whoever gave it is not told it is done. The task shows that
  its last turn ended with no report, and its owner is woken once and asked for one. If that turn says
  nothing either, whoever gave the task is told there is no report

### 5c. A truthful report is never refused

Parts: 5.4

- **Given** an agent's tasks in every state Gathr can put one in: waiting, ready, working, held,
  blocked, failed, stopped, done, cancelled
- **When** the agent reports the truth about each, from whichever turn it is in
- **Then** Gathr takes every report. A report on a task that is already done, or cancelled, is answered
  with what was recorded, not with an error

### 5d. What Gathr can see holds a task open, by name

Parts: 5.5, 5.6

- **Given** a task reported done while something is still open: its PR not merged, a wait, a task
  handed on, a background command, or a question
- **When** the report is taken
- **Then** the task is held and says on what. It becomes done when Gathr sees that thing end: the
  merge, the wait, the other task, the command, the answer. A task handed on in passing holds
  nothing, and is heard back on by itself

### 5e. Whoever gave a task hears back once, with the real outcome

Parts: 5.7

- **Given** a lead gave a task
- **When** the task settles: done, left, blocked, cancelled, or stopped by the person
- **Then** the lead hears back once, with that outcome and what to look at, and the hear-back names
  only what the lead has not heard yet. If the work carries on after a stop and then ends, the lead
  hears that outcome too. An outcome that changes after the lead heard it is heard once more

### 5f. A review is of an exact version

Parts: 5.8, 10.1

- **Given** a reviewer approved a PR at one commit
- **When** the PR gets a commit that only brings in its base, and then a commit that changes code
- **Then** the approval holds through the first and is out of date after the second, everywhere Gathr
  shows it. A merge through Gathr is refused unless what is merged has a clean review

### 5g. An update of Gathr doesn't stop work

Parts: 5.9

- **Given** a builder is in the middle of a turn on a task
- **When** Gathr updates itself and starts again
- **Then** the task is never marked stopped, nobody is told it stopped, and the builder carries on from
  where it was

### 5h. Stop now leaves the task as it was

Parts: 5.10

- **Given** a builder is in the middle of a turn on a task
- **When** the person presses Stop now
- **Then** the task keeps its state and shows that the person stopped it. Its lead is not told it
  settled. When the person presses Restart, the builder carries on

### 5i. "Left" stops the task and is told

Parts: 5.1, 5.5

- **Given** a builder working on a task, with nothing else open on it
- **When** it reports the task left, with what is left
- **Then** the task stops there and shows what is left. Whoever gave it hears that, and the builder is
  not started again by itself. If the task's PR merges later, the task is still not done: the builder
  is woken with what it left

### 5j. A hold that ends doesn't finish a task nobody reported

Parts: 5.2, 5.6

- **Given** a task held on something Gathr can see, with no "done" reported
- **When** that thing ends
- **Then** the task is not marked done. Its owner is woken on it and told what ended

## 6. The room explains itself

### 6a. Gathr can say who is doing what

Parts: 6.1

- **Given** a team where one agent works on a task, one waits, and one has nothing to do
- **When** the person, or an agent, asks Gathr who is doing what
- **Then** one answer from the service names each agent with its task and whether it is working,
  waiting or free. The window shows the same answer

### 6b. Gathr can say what the room is waiting on

Parts: 6.2

- **Given** a room where one task waits on CI, one on a question to the person, one on a teammate's
  review, one on a background command, and one turn is queued behind an update of Gathr
- **When** the person, or an agent, asks what the room is waiting on
- **Then** one answer lists every hold by name: what it is, who can end it, and how long it has
  waited. The window shows the same answer

### 6c. Gathr notices a quiet room before the person does

Parts: 6.3, 10.4

- **Given** work is owed and nothing is running or expected: a task ready with its owner free, or a PR
  reviewed and green that nobody is merging
- **When** that has lasted longer than the set time
- **Then** Gathr wakes whoever can act and tells the lead what was stuck and for how long, before the
  person has written anything. The time is recorded, so dead time has a number

### 6d. A turn that goes silent is seen

Parts: 6.3

- **Given** a builder's turn has made no progress for ten minutes
- **When** Gathr flags it
- **Then** the chat shows that turn as silent, with for how long, and its lead is told

### 6g. CI says what happened to each job

Parts: 6.2

- **Given** a wait on a PR's CI, where one job has passed and no runner has taken another
- **When** Gathr looks, and later GitHub cancels the job nobody took
- **Then** the wait says what it saw: one passed, and which job is queued. Its wake names the
  cancelled job as cancelled, never as failed, and a queued job is never called running

### 6e. Agents talk to each other in the open, as a thread

Parts: 6.4, 6.5

- **Given** a team chat
- **When** a builder @-names the reviewer in what it writes, and the reviewer answers
- **Then** the reviewer gets the message. The chat shows it as the builder's, with the @, and the
  reviewer's answer under it. The person reads both without opening anything

### 6f. Agreeing is a reaction, never a message

Parts: 6.6

- **Given** a builder got a message from the lead
- **When** all the builder has to say is that it agrees
- **Then** the lead's message shows a thumbs-up from the builder. No new message is posted, and nobody
  is woken by it

## 7. Everything in its place

### 7a. Each agent works in its own place

Parts: 7.1

- **Given** a project chat with a lead and two builders
- **When** each starts work
- **Then** each has its own worktree, in its own folder, on its own branch, and none of them is the
  person's own checkout

### 7b. A file written outside an agent's folders asks first

Parts: 7.2

- **Given** a builder on either engine, in an approval mode that asks
- **When** it writes a file into the person's own checkout, or anywhere outside its folders
- **Then** the person is asked, and nothing is written until they answer. If they refuse, the file
  isn't there

### 7c. A shell command's writes outside are stopped or caught

Parts: 7.3

- **Given** a builder on either engine
- **When** a shell command it runs writes a file into the person's own checkout
- **Then** either the write is stopped, or Gathr sees the new file when the command ends and tells the
  person and the lead which file and which agent. The checkout is never left changed with nobody told

### 7d. A worktree goes when its work is over

Parts: 7.4

- **Given** a builder's worktree for a PR
- **When** the PR merges or closes, or the chat is archived or deleted
- **Then** the worktree is removed, and the branch too once its PR has merged or closed. A worktree
  with work that was never committed is kept, and Gathr says so

### 7e. A short program ends with everything it started

Parts: 7.5

- **Given** Gathr runs a short program for an agent: git, gh, or a wait's check
- **When** the program ends or is stopped, having started other programs
- **Then** those end with it, and nothing it started is still running

### 7f. What an engine starts is known, and ends with its work

Parts: 7.6, 7.7

- **Given** a builder on either engine started a background command during a task
- **When** the task settles, or its engine closes
- **Then** Gathr listed the command while it ran, and the command and everything it started have
  ended

### 7g. An engine's temporary files go when it closes

Parts: 7.9

- **Given** an engine made temporary files while it ran
- **When** it closes
- **Then** those files are gone

### 7h. A test copy of Gathr can't be mistaken for your own

Parts: 7.8

- **Given** an installed Gathr opened on another data folder, beside the person's own
- **When** its window opens, and a new release is published
- **Then** its title names its folder, so it can't be read as the person's own, and it doesn't install
  the update by itself

## 8. Context lives in the record

### 8a. A task has a record

Parts: 8.1

- **Given** a task that was given, asked a question, got an answer, was reported left, then done, and
  was reviewed twice
- **When** its record is read
- **Then** it holds the brief, the question and its answer, both reports, both reviews with their
  commits, and the PR, in order

### 8b. Every turn on a task opens with its record

Parts: 8.2

- **Given** a task with a record
- **When** its owner is given a turn on it for any reason: the task itself, a wait that ended, an
  answer, a teammate's message, or carrying on after a restart
- **Then** the turn opens with the record: the brief, where it stands, and the questions, answers and
  reviews so far

### 8c. Gathr compacts when asked, and by itself at a set point

Parts: 8.3, 9.5

- **Given** an agent on either engine, on any model
- **When** the person presses Compact, or the agent's conversation reaches the set point
- **Then** it is compacted, and the chat says so with how full it is after. Before the set point the
  agent is told once to save what it still needs

### 8d. After a compaction an agent has what it needs back

Parts: 8.4

- **Given** a builder is partway through a task when its conversation compacts
- **When** its next turn starts
- **Then** it is given its role, the plan's state, its open tasks and the task's record, and it
  finishes the task from those alone

### 8e. After Gathr restarts, a turn that was cut off carries on

Parts: 8.5

- **Given** a builder is in the middle of a turn when the service crashes, or the computer restarts
- **When** Gathr starts again
- **Then** the turn carries on by itself. The task was never marked stopped, and nobody had to restart
  it

### 8f. An agent's conversation is kept across a restart

Parts: 8.6

- **Given** an agent has a conversation with its engine in a chat
- **When** the service stops and starts again, and the agent gets its next turn
- **Then** the engine takes up the same conversation

### 8g. An agent is told its role, and what the role does

Parts: 8.7, 11.1

- **Given** a new project on Gathr's defaults, with a lead, a reviewer and a builder
- **When** each starts its first turn
- **Then** each is told its team, its own role, and what that role does: the lead sizes and plans and
  is the one who asks the person, the reviewer checks plans and work it didn't make, the builder
  builds and reports. Nobody wrote a rulebook for it

## 9. Any engine, same behaviour

### 9a. Either engine in any seat

Parts: 9.1, 9.8

- **Given** the team scenarios on this page: a task given, reported, reviewed and merged
- **When** they run with Claude Code as lead, builder and reviewer, and again with Codex in each seat
- **Then** every one gives the same result

### 9b. Both engines get the same tools and the same briefing

Parts: 9.2

- **Given** one agent on Claude Code and one on Codex, in the same seat of the same team
- **When** each starts a turn
- **Then** each is offered the same Gathr tools and is given the same briefing

### 9c. Approvals behave alike

Parts: 9.3

- **Given** one agent on each engine, in the same approval mode
- **When** each writes a file outside its folders, writes outside with a shell command, and pushes to
  the default branch
- **Then** each action has the same outcome on both: the person is asked, or it is allowed, or it is
  refused

### 9d. The model list is each engine's own answer

Parts: 9.4

- **Given** an account on each engine
- **When** the person opens the model list for an agent
- **Then** it shows the exact models that engine reports for that account, and no "latest" or family
  entry

### 9e. Gathr reaches GitHub for agents

Parts: 9.6

- **Given** an agent whose engine has no GitHub sign-in of its own
- **When** it reads a PR, reads its CI, brings it up to date and merges it, with Gathr's tools
- **Then** each works, through the person's own sign-in on the computer, and the agent never holds
  that sign-in

### 9f. A review by a different provider

Parts: 9.7

- **Given** the person's agents are on two providers
- **When** the assistant proposes a team
- **Then** the reviewer it proposes is on a different provider from the builders. With agents on one
  provider only, it still proposes a reviewer and says the review is by the same provider

## 10. Quality and speed in balance

### 10a. Proof is never skipped

Parts: 10.2

- **Given** a project with checks, and a PR reviewed clean
- **When** the reviewer merges it through Gathr
- **Then** Gathr has run the checks at the exact commit and kept the result, where the reviewer and
  the person can read it. Where the project requires its checks to pass, the merge is refused while
  one has failed or hasn't run at that commit

### 10b. A PR behind its base is brought up to date before it merges

Parts: 10.5

- **Given** a PR reviewed clean, and its base has moved since
- **When** it is merged through Gathr
- **Then** Gathr brings it up to date first, its review still holds
  ([5f](#5f-a-review-is-of-an-exact-version)), and it is never merged while it is behind

### 10c. The size of the process fits the size of the ask

Parts: 10.3

- **Given** an everyday ask and a new ask in the same project
- **When** each is carried out
- **Then** the everyday ask made no plan, no task for the person and no review. The new ask went
  through every step of the loop in order, and none could be skipped

### 10d. Work that isn't code is checked, and saved where it belongs

Parts: 10.1

- **Given** a task whose result is a document, with a brief that says where it is to be saved
- **When** its builder reports it done, naming the file
- **Then** someone who didn't write it checks that version before it counts as landed. Landing is
  the file saved in the place the brief named, and the person is shown where

## 11. No rulebook needed, and Gathr measures it

### 11a. A new project works on defaults

Parts: 11.1

- **Given** a new project with no hand-written `AGENTS.md`, on Gathr's defaults
- **When** the person asks for something new
- **Then** [the project run](#r2-the-project-run) passes, and no step in it needed a rule somebody
  wrote for this project

### 11b. Gathr counts the person's corrections

Parts: 11.2

- **Given** the room did something the person had to correct
- **When** the person marks a message or a task "I had to correct this" or "this should have been
  asked"
- **Then** Gathr records it with its day and what it was about, and the day's count goes up by one

### 11c. A scorecard in the app, from Gathr's own events

Parts: 11.3

- **Given** a project the room has worked in for several days
- **When** the person opens the scorecard
- **Then** it shows the corrections counted for each day, beside the other numbers Gathr counts. Every
  number comes from Gathr's own events: none was typed in

### 11d. Friction is recorded by Gathr, not by hand

Parts: 11.4

- **Given** something in Gathr got in an agent's way during a task, and something else in the
  person's
- **When** the agent says so in its report, and the person marks theirs
- **Then** Gathr keeps each one, with who reported it, when and on which task, where the person and
  the lead can read them all. Nobody copied anything by hand

## 12. Open both ways

### 12a. An outside AI the person allowed brings something, and hears back

Parts: 12.1, 12.2, 12.3

- **Given** the person allowed one outside AI app to reach a project's team
- **When** that app sends the team an ask
- **Then** the ask shows in the chat as from a guest, by its name, and goes to the lead. The lead's
  answer returns to the app over the same connection

### 12b. An outside AI the person hasn't allowed is refused

Parts: 12.1

- **Given** an outside AI app the person hasn't allowed
- **When** it sends an ask
- **Then** it is refused, nothing shows in any chat, and the attempt is on record

### 12c. Nothing from outside speaks as the person

Parts: 12.5

- **Given** a guest is connected to a team
- **When** it sends a message, tries to approve a plan, and tries to answer a question or an approval
- **Then** its message shows as the guest's, never the person's, and each of the others is refused

### 12d. An agent reaches an outside tool the person allowed

Parts: 12.4

- **Given** the person allowed one outside tool for an agent, in Settings, and not another
- **When** the agent uses each
- **Then** the first works and the second is refused, saying it isn't allowed. What each agent may reach
  shows in Settings

## The two runs

The whole product, end to end. They are the last scenarios to pass.

### R1. The everyday run

- **Given** a person with an assistant and one specialist, and no project
- **When** they start a new chat, name nobody, and ask for something small: "summarise this file", with
  the file attached
- **Then** all of this holds:
  1. The assistant gets the ask ([1a](#1a-a-new-chat-with-nobody-named-is-with-the-assistant)), and it
     is sized as everyday ([2a](#2a-every-ask-is-sized-and-gathr-holds-the-size)).
  2. The assistant does it with its own tools and answers in the chat.
  3. No team was proposed, no plan was made, no task reached the person, and nothing was reviewed
     ([10c](#10c-the-size-of-the-process-fits-the-size-of-the-ask)).
  4. The person was asked nothing.

Run a second time with an ask that belongs to the specialist: the assistant hands it on
([1e](#1e-the-assistant-hands-an-ask-to-the-specialist-it-belongs-to)) and the answer still arrives in
the same chat.

### R2. The project run

- **Given** a person with a staff of agents on two providers, a new project on Gathr's defaults, and
  no hand-written rulebook
- **When** they type "build an export for my notes"
- **Then**, in this order, each step held by Gathr:
  1. **Size.** The ask is sized as new ([2a](#2a-every-ask-is-sized-and-gathr-holds-the-size)).
  2. **Team.** The assistant proposes a team, with a lead and a reviewer on another provider, and
     nothing is planned until the person agrees it
     ([1f](#1f-a-projects-team-is-proposed-and-agreed-before-any-planning),
     [9f](#9f-a-review-by-a-different-provider)).
  3. **Plan.** The lead asks what it must, one question at a time, then writes a plan in parts
     ([2e](#2e-the-lead-asks-its-questions-first-one-at-a-time)).
  4. **Check.** The reviewer checks the plan against the code before the person sees it
     ([2c](#2c-no-plan-reaches-the-person-unchecked)).
  5. **Approve.** The person approves. Before that, no task could be given
     ([2b](#2b-nothing-new-is-handed-out-before-its-plan-is-approved)).
  6. **Hand out.** Gathr makes one task for each part, each with a complete brief
     ([3b](#3b-approving-a-plan-makes-its-tasks), [3a](#3a-a-task-carries-a-complete-brief-or-it-isnt-given)).
  7. **Build.** Each builder works in its own place and reports with what to look at
     ([7a](#7a-each-agent-works-in-its-own-place), [5a](#5a-the-owner-reports-a-task-done-left-or-blocked)).
  8. **Review.** Someone who didn't make it approves it at an exact commit
     ([5f](#5f-a-review-is-of-an-exact-version)).
  9. **Land.** Gathr runs the checks and merges ([10a](#10a-proof-is-never-skipped)). The worktrees go
     ([7d](#7d-a-worktree-goes-when-its-work-is-over)).
  10. **Prove.** The lead shows the person the export working, with proof.
  11. **Learn.** The friction the agents reported in the run is on record
      ([11d](#11d-friction-is-recorded-by-gathr-not-by-hand)), and the run's numbers are on the scorecard
      ([11c](#11c-a-scorecard-in-the-app-from-gathrs-own-events)).
  12. **And throughout:** only the lead asked the person anything
      ([4a](#4a-a-members-question-goes-through-the-lead-in-the-leads-words)), no task closed without a
      report ([5b](#5b-a-turn-that-ends-is-not-a-task-that-is-done)), and the person never had to ask
      what the room was waiting on ([6c](#6c-gathr-notices-a-quiet-room-before-the-person-does)).

## What a replay can't prove

A replayed engine says what it was recorded saying. It proves what Gathr does with that. It can't prove
what a real model will choose. These are checked with real engines, by a person, when their promise is
built:

- That the assistant does a small thing itself, and hands on only what belongs to a specialist (1a, 1e,
  R1).
- That a lead sizes an ask rightly, and asks only what it couldn't find for itself (2a, 2e).
- That a reviewer's check of a plan finds what is wrong with it (2c).
- That an agent with only the brief does the task right, and one that was compacted finishes it right
  (3d, 8d).
- That the default briefing is enough for each role to work as described (8g, 11a, R2).

## Not settled yet

What these scenarios had to assume. Each is decided in the plan for its promise, and the scenario
changes with it:

- **A member's tool approval** (4a). Today it goes straight to the person. Whether that counts as the
  member asking the person isn't decided.
- **Full approvals** (7b). In Full, nothing asks before a write outside an agent's folders. Whether Full
  should still protect the person's own checkout isn't decided.
- **Where a hand-over shows** (1e). In the chat the person started, as written here, or in a chat of
  the specialist's.
- **How long a quiet room is left** (6c) before Gathr acts.
- **Tools that answer for the person** (12c). Agents hold tools that can answer an approval or a
  question when the person asks them to. A guest gets none of them; what an agent may do with them
  isn't decided.
