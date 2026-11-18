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
- **When** they start a new chat, type @ and pick the other agent from the list
- **Then** that agent gets the message, the assistant doesn't, and the chat is listed under that
  agent's name; the same name typed out without picking is plain text and goes to the assistant

### 1c. In a team chat, @Name reaches that member

Parts: 1.3

- **Given** a team chat with a lead and a builder, open in the window
- **When** the person sends one message with the builder picked from the @ list, one that names nobody,
  and one with `@Builder` typed out without picking
- **Then** the builder gets the first and the lead gets the other two, as the box said by Send ("To" and
  the agent's name)

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

- **Given** a team chat with a lead
- **When** the person reports something broken, and then asks for a new feature
- **Then** each ask has a size on record: repair or new, and the window shows it as a line, "Fix" or
  "Plan", after the lead's turn that sized it. The repair gets a short brief. The feature starts a
  plan. An ask the lead leaves unsized is treated as new: its line reads "Plan" once it has a task
  or Gathr has held something of it (2b), and it has no line before then

### 2b. Nothing new is handed out before its plan is approved

Parts: 2.2

- **Given** an ask sized as new, or not sized, in a team chat, and its plan not yet approved
- **When** the lead, or any member, gives a work task for it, and then gives a look-into task and a
  review
- **Then** Gathr refuses the work task and says what to do instead, by the state of the ask's plan: size
  it a repair, write the plan, wait for its check or for the person's Approve, or write the next version.
  The look-into task and the review are taken. The ask's line, "Plan", shows after the lead's first
  reply below the person's message, and opens to "Just fix it." and "Plan it with me first.". The same
  work task is accepted once the person has approved the plan, or the ask is a repair

### 2c. No plan reaches the person unchecked

Parts: 2.3

- **Given** the lead has written a version of a plan with the `plan` tool
- **When** Gathr gives the check to the reviewer
- **Then** the version is the lead's own message in the chat, with a chip, and its tab says it is being
  checked: there is no Approve. A thumbs-up from the checker on that message agrees. Gathr's line says
  how the check ended ("Bo agrees", "3 findings" or "Bo didn't finish"), and the chat then needs the
  person. A check that left findings wakes the lead, who answers each in its next version, and only
  the one who objected checks that version, on those points. A check still open 10 minutes after the
  version was written reaches the person as "didn't finish"

### 2d. A plan is a record with versions and states

Parts: 2.1

- **Given** a plan the person sent back with words
- **When** the lead writes the next version
- **Then** Gathr holds both versions, the words and the lead's answers. The new version is checked again
  before the person sees it, and the old one reads as replaced. From its first version a plan is in one
  state at every moment, read from its rows: checking, revising, waiting on the person, approved or
  dropped. A second plan for an ask keeps the first

### 2e. The lead asks its questions first, one at a time

Parts: 2.5

- **Given** an ask sized as new, where two things must be decided before a plan can be written
- **When** the lead asks the person
- **Then** the first question arrives alone, with options and the lead's pick. The second arrives after
  the first is answered. Both, with their answers, are held with the plan, and the plan can't be
  written while a question is open

### 2f. A repair doesn't wait for the person, and is shown after

Parts: 2.6

- **Given** an ask sized as a repair
- **When** the lead gives its short brief
- **Then** the task is handed out with no plan and no approval, and is built and reviewed. When it
  lands, the person is shown "Fixed: <the ask>", which opens to its PRs and proof, and can change it
  to Plan, which Gathr counts. Nothing of the repair is held: its turns, gives, writes and pushes go
  ahead

### 2g. A held turn can't change the project's files or push

Parts: 2.2

- **Given** a project chat with a lead and a member, an ask sized as new with no approved plan, and the
  lead's turn running on Claude Code
- **When** the lead writes a file in its worktree, then a note in its own folder and another in Shared,
  and then pushes
- **Then** the write into the worktree is denied, in every approvals mode, Full included and with no
  question to the person, with a reason that names the worktree, the agent's own folder and Shared; for
  the lead the reason ends by what its plan needs (size a repair, wait for the check, wait for the
  Approve). The notes are written. The push is refused with "held: <the ask> has no approved plan, so
  nothing is pushed for it", on either engine. When the person chooses "Just fix it.", the same write
  goes on. A write the hold can't be read for goes on, and says why in Diagnostics. A shell command's or Codex's change to the
  worktree, which Gathr can't refuse, is found at the turn's end: the chat shows one line in that turn,
  "Lead changed 1 file before the plan was approved", opening to the file (a build into an ignored
  folder and a pull or a checkout to read something add nothing), and the plan's tab shows it as
  Changed. The agent's next turn opens with "In your last turn you changed 1 file in <worktree>: new.txt.
  The person sees them." and the reason for the hold, naming the worktree, its own folder and Shared.
  A turn released by "Just fix it." before its end reports nothing, and a look that fails says why in
  Diagnostics and finds nothing.

### 2h. A repair far past its Fix's line stops and asks, before its review

Parts: 2.4, 10.3, 10.7

- **Given** a project chat with a lead, a builder and a reviewer, and a work task #1 on a repair ask, with the
  builder's turn running
- **When** the builder reports #1 done naming PR #5, whose head adds 1,240 lines
- **Then** Gathr asks the person once, in the builder's turn: "Gathr asked you: #5 adds 1,240 lines, over a Fix's line
  of 1,000: let it go on?", with the numbers and the largest files, and the builder is told its review waits for the
  answer. No review task exists, and #1 is held as `done, waiting on #<question> (the person), #5 to merge`. Reporting
  done again asks nothing more
- **When** the person picks "Send it back"
- **Then** #1 is blocked and the lead hears. Reported done again at the same head, it is blocked again with no review.
  Reported done at a new head that adds 300 lines, it has no question and its review is given
- **When** instead the count fails because GitHub doesn't answer, or takes over 20 seconds
- **Then** the report goes on as it always did: the review is given, the failure is in Diagnostics under the report's
  reference, and landing counts the PR against the same line

### 2i. A plan part far past its size stops and asks once

Parts: 2.4, 10.3, 10.7

- **Given** a work task #1 from an approved plan whose part said 100 lines, its PR #5 reported done
- **When** the head adds 500 lines
- **Then** Gathr asks once: "#5 adds 500 lines, over its part's line of 400: let it go on?", saying the part said 100
  lines. The review isn't given
- **When** the person picks "Let it go on"
- **Then** #1 waits on its merge alone and the reviewer's review starts. When the reviewer reports done, the PR lands
  and Gathr doesn't ask again for those 500 lines

### 2j. The person changes a running repair to new

Parts: 2.4

- **Given** an ask sized as a repair, with work running
- **When** the person opens its size line and chooses "Plan it with me first."
- **Then** Gathr records the new size and that the person set it, and the line reads "You: Plan". The
  lead can't change it back. Changing a repair to new is counted as a correction. Its tasks that
  haven't started wait for the plan ("#5 (Ann) waits for its plan"), the lead is told, and the lead
  isn't woken for the tasks that wait. A task already working goes on, and its turns aren't held. When
  the person changes it back to a repair, those tasks go back in line

### 2k. A follow-up joins the ask it follows

Parts: 2.4

- **Given** an ask sized as a repair, with work open
- **When** the person sends "also check X"
- **Then** it joins the repair: no new ask is made, and the chat shows no second size line. The lead's next
  gives and writes are not held. When the lead calls `size` with that message, it becomes an ask of its
  own, with its own size and its own line

### 2l. A look-into task is held for its whole life

Parts: 2.2

- **Given** an ask sized as new with no approved plan
- **When** the lead gives a member a look-into task (`look_into`), and the member works on it, writes a
  note in its own folder, tries to give a work task and then reports done with what it found
- **Then** the task is taken, with a brief that says it only looks into something, and "Task #12 · Look
  into" heads its view. The member's write into its worktree is denied and its notes are written. Its
  work task is refused, and a look-into task it gives is taken. Its done report, with a note or a file,
  reaches the lead as any task's does, and names no PR. After the plan is approved, the task's write
  into the worktree is still denied. A shell command's change to the worktree is found at
  the turn's end and shown as "<member> changed 1 file on look-into task #12".

## 3. Every task stands on its own

### 3a. A task carries a complete brief, or it isn't given

Parts: 3.1, 3.2, 3.3, 3.4

- **Given** an approved plan, or a repair
- **When** a task is given
- **Then** it carries what to do, why, what done means, how it's proved, and where the work lives: the
  project, the folder and the branch. A task that lacks one of these is refused, naming what is
  missing. Its brief says each of them, and its size when given

### 3b. Approving a plan makes its tasks

Parts: 3.5

- **Given** a checked plan in three parts, each with an owner, what done means, how it's proved and
  its order
- **When** the person approves it
- **Then** Gathr makes three tasks in one step, each with its part's brief, owner and order, word for
  word, and starts the ones with nothing to wait on. No turn of the lead's is spent on it, and the lead
  hears back on the tasks as on any it gave. Nobody types them again

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

### 3f. The giver corrects a task before it starts

Parts: 3.8

- **Given** a task waiting for its first turn
- **When** its giver changes its title and what it comes after
- **Then** its brief and its order follow, and the task shows the change. Once its first turn has
  started, a change is refused, with the reason

### 3g. A PR's review is given by Gathr

Parts: 3.6, 10.8

- **Given** a chat with a Reviewer, and a builder with a work task
- **When** the builder's done report first names PR 7
- **Then** the Reviewer gets one review task for PR 7, given as the builder, which starts when the
  builder's work is held only on its merge. A review of it given while that one is open is refused,
  saying which review is open and who gave it. A second report naming it makes none and says nothing
  more

## 4. One voice to you

### 4a. A member's question goes through the lead, in the lead's words

Parts: 4.1, 4.3

- **Given** a team with a lead and a builder
- **When** the builder needs an answer from the person
- **Then** the builder's ask becomes a request the lead owns, and the builder never asks the person
  directly. The lead answers it when it can. Otherwise the question that reaches the person is
  text the lead wrote: Gathr refuses to forward the builder's text as it is

### 4b. A question is a record, with the lead's pick first

Parts: 4.2

- **Given** the lead asks a question with three options and names its pick
- **When** the person opens it
- **Then** the pick is first and marked as the lead's pick. The person can choose an option, answer
  in their own words without choosing one, or Discuss it with the lead. An agent hears the pick and
  the words apart. The question, its options, the pick and the answer are held

### 4c. An answer reaches everyone who waits on it, at once

Parts: 4.4, 4.5

- **Given** a builder's task waits on a question the lead put to the person
- **When** the person answers
- **Then** the builder gets the answer and carries on with no turn of the lead's in between, and the
  lead is told too. Until the answer comes, the builder's task stays open and says it waits on the
  question, and the builder isn't told it's blocked

### 4e. The person discusses a question with the lead

Parts: 4.2, 4.4

- **Given** a question the lead asked for a builder, open on the person's list
- **When** the person turns on Discuss, writes a message and sends it
- **Then** the question leaves the person's list and the message reaches the lead alone, as its next
  turn. The builder keeps waiting and hears nothing. When the lead's turn ends without a new question,
  the question is back on the list as it was. When the lead asks a new question in that turn, the new
  one takes the old one's place

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

- **Given** a lead gave tasks, in three turns of one round, and a builder gave its PR's review
- **When** the tasks end: one blocked while others run, the rest done; the review comes back clean
- **Then** the lead is woken at once for the blocked one, by itself. For the rest it hears once, when
  all of the round is in, in a turn that answers the newest of the three, and the hear-back names
  only what the lead has not heard yet. Work from a later message of the person's doesn't hold that
  back. The builder gets no turn for its clean review: its task's reason names only its merge. A
  second task of the builder's that it never reported done gets its hear-back turn. Each task that
  ends done is a line in the chat when it ends, shown once. The window's plan of the round reads
  the round from the service and shows it finished. If the work carries on after a stop and then
  ends, the lead hears that outcome too. An outcome that changes after the lead heard it is heard
  once more. A task given in passing is heard by itself, and the person's answer still wakes at once

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
  waiting or free, a free agent included. An agent with an approval waiting and a turn running
  reads as needing the person. The window shows the same answer
- **And** an agent whose last turn failed and wasn't retried, or that waits for a sign-in, reads can't run; one the
  person stopped or paused, or whose turn a restart cut, reads stopped; one whose task is blocked reads blocked; a lead
  whose plan waits on the person needs them; each says since when. Every face of each agent in the chat carries that
  one state's mark, and pointing at a face, or focusing its button, says the state, what and since when

### 6b. Gathr can say what the room is waiting on

Parts: 6.2

- **Given** a room where one task waits on CI, one on a question to the person, one on a teammate's
  review, one on a background command, and one turn is queued behind an update of Gathr
- **When** the person, or an agent, asks what the room is waiting on
- **Then** one answer lists every hold by name: what it is, who can end it, and how long it has
  waited. The window's status line says each agent's state from the same answer, a wait on one thing by
  name, and changes without a reload; it says nothing once the last hold ends

### 6c. Gathr notices a quiet room before the person does

Parts: 6.3, 10.4

- **Given** a team chat where work is owed and nothing moves: a task ready with no turn on it, nothing
  that moves by itself, and nothing that waits on the person
- **When** Gathr checks the chat, and it stays so
- **Then** the lead is woken once, on no task, with what isn't moving and for how long, before the
  person has written anything. Once the chat has been stuck 5 minutes in all, the person is told in
  a line in the chat, once, and the chat is marked as needing them. A PR left to its reviewer wakes
  its reviewer and not the lead as well. When something moves, or the person writes, the mark goes
  and the stretch is recorded with its length, what wasn't moving and what ended it, so dead time
  has a number. If the same task still isn't moving after they wrote, the person isn't told of it
  again, however long it lasts. A second thing that stops moving gets its own 5 minutes: then the
  person is told, in a line that names the new thing first. A task that is restarted and fails
  again is a new thing; one whose giver has since heard of it is not

### 6d. A turn that goes silent is seen

Parts: 6.3

- **Given** a builder's turn on a task has made no progress for ten minutes, or twenty while a tool
  is open, on either engine
- **When** Gathr looks
- **Then** the chat shows that turn as silent, with for how long, in its agent's row of the status
  line's list, and its lead is told once for that turn. The turn is never stopped, and reads as
  working again once it makes progress. A turn that waits for an approval is never silent, however
  late the answer comes. A lead's own silent turn wakes nobody: if nothing else moves, the person is
  told at 5 minutes

### 6h. A quiet room is noticed once, and only when it is quiet

Parts: 6.3

- **Given** chats with work owed: one whose lead's wake changes nothing; one whose trouble its lead
  has already heard; one that waits only on the person; one whose task's owner is paused; a paused
  chat; an archived chat; one that waits on a queued CI job; one whose task its lead restarts, and
  which fails again, three times over; and one where the lead was woken for a task, the person
  wrote, and a second and a third task then stop moving
- **When** Gathr checks each again and again, and the service is stopped and started in between
- **Then** a wake that changes nothing isn't repeated, and the person is still told at 5 minutes. A
  lead that already heard the trouble isn't woken again. The lead is woken again only for what
  wasn't in its last wake, named first, and never while that wake is still queued or running: the
  person's message doesn't make the first task news again. The chat that waits on the person, the one
  whose owner is paused, the paused one, the archived one and the one waiting on CI are never told
  as stuck. After the restart the lead isn't woken twice, and the time the service was down isn't
  counted. The task that fails again and again still gets the person told, once 5 minutes of stuck
  time have added up

### 6g. CI says what happened to each job

Parts: 6.2

- **Given** a wait on a PR's CI, where one job has passed and no runner has taken another
- **When** Gathr looks, and later GitHub cancels the job nobody took
- **Then** the wait says what it saw: one passed, and which job is queued. Its wake names the
  cancelled job as cancelled, never as failed, and a queued job is never called running

### 6e. Agents talk to each other in the open, as a thread

Parts: 6.4, 6.5

- **Given** a team chat
- **When** a builder sends the reviewer a message, and the reviewer answers it
- **Then** the reviewer gets the message. The chat shows it as the builder's, opening with the
  reviewer's @Name, in a thread under what the builder was writing, and the reviewer's answer under it
  in the same thread, even when the person wrote something in between. The person reads both without
  opening anything, and can fold the thread

### 6f. Agreeing is a reaction, never a message

Parts: 6.6

- **Given** a builder got a message from the lead
- **When** all the builder has to say is that it agrees, it reacts to the id the message carried
- **Then** the lead's message shows a thumbs-up and a count, with the builder's name when it is
  pointed at or focused. No new message is posted, and nobody is woken by it. The window shows it at once

### 6i. An agent reads why something failed, in its own chat

Parts: 6.7

- **Given** a tool call that failed in a chat with its reference, a task blocked with a secret in its
  reason, a failure in another chat, and a turn of many tool calls
- **When** an agent asks the `diag` tool for the trace, the failures and health
- **Then** the forced failure is found from its reference. The secret comes back hidden. The other
  chat's events don't come back, and aren't counted. A long answer is cut at the cap and says so,
  with the failure first

### 6j. When a chat needs you, you're told outside the window

Parts: 6.8

- **Given** the window is hidden
- **When** a chat starts needing the person, or something new starts waiting on them in a chat that
  already does
- **Then** one notification is decided for it, with the chat's title and what now waits on them. None
  while the window is in front. None a second time for that chat before the window has been in front,
  if only for a moment between reads, and then one for what came after. None during an app-wide
  pause, except a sign-in and Gathr's own ask. One summary on resume, and one at a start with chats
  already needing them

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

### 7i. A deleted project or agent leaves nothing of Gathr's behind but Shared

Parts: 7.10

- **Given** a project with chats, one of them running a turn, and an agent in two projects with open
  tasks, the lead of one and the only agent of another
- **When** the person deletes the project, then the agent
- **Then** the project goes with its chats and every folder Gathr made for it but Shared, and the
  person's checkout stays as it was; the agent's tasks are cancelled, the next agent leads where it
  led, what it was alone in goes, and its old messages still name it
- **And** a folder that can't go yet is named with its reference, and goes at a later sweep

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
- **Then** each is told its team, its own role, and the few things that role must do that Gathr's tools
  don't already make it do, each shipped only after the trial on real engines showed it changed what
  agents do ([11f](#11f-a-line-ships-only-when-it-did-clearly-better-with-it-than-without)): the
  builder to make the smallest change that does what was asked, and all three to agree with a
  thumbs-up. The lead and the reviewer have no line of their own. Sizing, plans, who asks the person,
  checks and reports come from Gathr's tools and state. Nobody wrote a rulebook for it

### 8h. A member's second task starts in a new conversation

Parts: 8.8

- **Given** a lead, Cass and Pia; Cass has done #1 and #2 and has 5 more tasks in the chat
- **When** the lead gives Cass #8
- **Then** all of this holds:
  - #8 starts a new conversation, not a resume
  - its first input opens with "This is a new conversation for task #8.", then 5 task lines, newest first, without #8
  - Cass's context is empty until its first call
  - a turn of the lead's on its own task closes no engine and opens no new conversation

### 8i. It goes back to its first task's conversation

Parts: 8.8

- **Given** Cass worked #1, then #2
- **When** a restart of #1 is given
- **Then** all of this holds:
  - the engine closes with "the next turn is on another task"
  - it starts by resuming #1's conversation
  - there is no opening
  - #1's saved fill shows again

### 8j. A hear-back and a fix go back to the work

Parts: 8.8

- **Given** Cass, on #1, gave #3 to Pia; #1 ended; Cass worked #2
- **When** #3 settles
- **Then** Cass's hear-back resumes #1's conversation, and a teammate's message naming "#1" runs in #2's
- **Given** Cass's work task #4 on PR 7 and a review with notes
- **When** Gathr gives the fix #6
- **Then** #6 resumes #4's conversation; a fix whose work task has none starts new

### 8k. A conversation that can't be resumed starts again from the brief

Parts: 8.8

- **Given** #1's row, with a conversation the engine refuses (Claude Code or Codex)
- **When** a turn on #1 starts
- **Then** all of this holds:
  - `gathr.engine.resume_failed` is on that turn's trace
  - the row is gone
  - the same turn runs in a new conversation, with #1's brief once
  - the turn ends done

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
- **When** Gathr lands it
- **Then** Gathr has run the checks at the exact commit and kept the result, where the reviewer and
  the person can read it. Whatever the project requires (Gathr's checks, GitHub's CI, or both) holds
  the PR until it passed at that commit. Where it requires none, the review lands the PR once no CI
  run is still going, and a CI failure leaves the PR to its reviewer

### 10b. A PR behind its base is brought up to date before it merges

Parts: 10.5

- **Given** a PR reviewed clean, and its base has moved since
- **When** Gathr looks at it
- **Then** Gathr brings it up to date, its review still holds
  ([5f](#5f-a-review-is-of-an-exact-version)), and it lands when its checks pass again at the new
  commit. It is never merged while it is behind

### 10c. The size of the process fits the size of the ask

Parts: 10.3

- **Given** a repair and a new ask in the same project
- **When** each is carried out
- **Then** the repair made no plan and no question for the person. The new ask went
  through every step of the loop in order, and none could be skipped

### 10d. Work that isn't code is checked, and saved where it belongs

Parts: 10.1

- **Given** a task whose result is a document, with a brief that says where it is to be saved
- **When** its builder reports it done, naming the file
- **Then** someone who didn't write it checks that version before it counts as landed. Landing is
  the file saved in the place the brief named, and the person is shown where

### 10e. A PR reviewed clean lands by itself

Parts: 10.4

- **Given** a PR whose newest review in the chat is done on a commit that covers its head
- **When** what the project requires has passed there and no CI run is still going
- **Then** Gathr merges it, with no agent's turn. It doesn't land on an older approval while a newer
  review is open, after an author's commit past the review, into a branch that isn't the default, or
  without its reviewer's sign-off where it changes tests or check settings. Sign-off given with the
  review lands it

### 10f. A PR that can't land says why, once

Parts: 10.4

- **Given** a PR reviewed clean that Gathr can't land: its update conflicts, a check the project
  requires failed, or CI failed where it isn't required
- **When** Gathr looks at it, again and again, and after a restart
- **Then** its reviewer is woken once for that commit, on no task, with what Gathr saw, and its
  review stands. A merge that GitHub doesn't confirm settles nothing and isn't made again

### 10g. A review asks what fails and what wasn't asked

Parts: 10.6

- **Given** a PR whose builder was asked for something small, and a reviewer given its review
- **When** the reviewer finds code the ask doesn't need, and reports the review left with an `unasked` note
- **Then** the PR isn't landed, and the builder is given a task to take it out, in passing, with the note in its
  why. The reviewer's giver hears that the review is blocked and who has the fix. A done review says
  `nothing_unasked: true` and has no `wrong` or `unasked` notes, or it is refused with a reference. Its
  suggestions are heard with the done line and start no turn

### 10h. A change past the project's far end waits for the person

Parts: 10.7

- **Given** a project whose main has 9 changes of 100 lines each, a lead, a builder and a reviewer, and a work task
  #1 "Add a retry" from the lead to the builder, with PR #5
- **When** the builder's head adds 2,000 lines in `src`, 9,000 in `Cargo.lock`, a png, and 500 lines in a file the
  base's attributes mark generated, and its review is reported done
- **Then** Gathr counts 2,000 lines against a line of 3,000 and lands it. Its row says so, with no question
- **When** the builder's next head adds 3,200 lines and its review is reported done
- **Then** Gathr doesn't land it. The person has one question from Gathr, with 3,200, 3,000, the largest file and
  "It answers task #1 from" the lead. Task #1 is unchanged and no turn starts. The waiting answer says `#5 to land:
  waits on your answer to #<question>`, and it waits on the person. The merge tool refuses with a reference
- **When** the builder, and then the person in words alone, answer the question
- **Then** both are refused, each with a reference: only the person answers it, with one of its choices
- **When** the person picks "Let it go on"
- **Then** the PR lands without waiting for the next look. Its row is the record: when it landed, 3,200 counted,
  3,200 let through, landed by Gathr, the builder and the reviewer each with their engine, the commit its first
  review named, and its `unasked` notes
- **When** instead the head grows to 6,201 lines before the person answers, and they pick "Let it go on"
- **Then** a second question is filed and it doesn't land. At 6,200 it lands
- **When** instead the person picks "Send it back"
- **Then** #1 is blocked with "sent #5 back", and one hear-back turn is queued for the lead. #5 doesn't land, and
  a new review at that head changes nothing: its waiting line moves others, so the lead isn't woken for it. The
  lead's restart of #1 carries the reason into its brief
- **When** instead the question is open and the builder pushes a head that adds 500 lines, and its review is
  reported done
- **Then** landing cancels the question as no longer needed and lands the PR
- **When** instead the PR's work task has a lower line than the project's (a Fix's 1,000, or its part's), and the
  head grew past it after the task reported done
- **Then** landing measures against that lower line, and asks with it ("over a Fix's line of 1,000"). A count the
  person let through when the task reported done isn't asked about again
- **When** instead the question is open and the PR is merged outside Gathr
- **Then** the question is cancelled, not answered. The row is the record, landed by hand, with its reviewer
- **When** a push Gathr made and GitHub didn't confirm is seen merged later
- **Then** the record says Gathr landed it
- **When** main has 49 changes of 100 lines and one of 9,000
- **Then** a PR adding 9,000 lines lands and one adding 9,001 is asked about
- **Left out, on the owner's word** ([0054](decisions/0054-a-change-past-the-far-end-waits-for-the-person.md)):
  a question is asked at landing, not when the PR appears, and no turn on its tasks is held while it is open

### 10i. A merge brings the project folder along

Parts: 10.9

- **Given** a project whose folder is on its default branch, with a change to a file the PR doesn't touch
- **When** Gathr merges a PR into the default branch, by fast-forward, merge commit or squash
- **Then** the folder is fast-forwarded to the merged commit, and the change is still there
- **When** instead the folder is on another branch, has commits `origin` doesn't, is mid-rebase or mid-cherry-pick (even with its first conflict committed), or has a change to a
  file the merge touches (even with `merge.autostash` on), or the PR went into another branch
- **Then** the folder is left as it was, and the chat has one line from Gathr saying why, with a reference that
  traces to the merge

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

### 11e. Gathr's words to agents are measured

Parts: 11.5

- **Given** a team chat
- **When** the person writes, the lead gives a member a task, the member's turn ends with no report after
  one turned-away report, and Gathr asks once
- **Then** each turn records which words opened it and their version. `gathr diag words` counts, for each
  kind and version, its turns, its turns with no tool call, the `finish_task` calls turned away, and the
  task turns that ended with no report. A change to a kind's main form without a new version fails a test

### 11f. A line ships only when it did clearly better with it than without

Parts: 11.1, 8.7, 6.6

- **Given** a line of role text and the case that measures it, on a made-up project, on the engines the
  room uses for that role
- **When** the case runs with the line and without it, alternating block by block, on one build and one
  set of engine copies, and Gathr's records and the diff are counted: no model judges another
- **Then** the line ships only if the runs with it did clearly better on every engine, and a line that
  made no clear difference is left out, with its case. Each shipped line has its text's hash and a
  `kept` decision in the trial's committed file, and a test fails for a line that has none. Of the four
  lines tried, the member's and the thumbs-up shipped; the lead's and the code review's made no
  difference and were removed

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
  1. The assistant gets the ask ([1a](#1a-a-new-chat-with-nobody-named-is-with-the-assistant)). Sizing
     an everyday ask is the assistant's, written with a later stage.
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
- What a line of role text does to a real model: the trial in `docs/testing.md` runs it on real engines
  by hand (11f), and a replay never does.
- That the system shows a notification, that a click on it opens the chat, and that Claude Code passes
  the hook's reason back word for word (6j).

## Not settled yet

What these scenarios had to assume. Each is decided in the plan for its promise, and the scenario
changes with it:

- **A member's tool approval** (4a). Today it goes straight to the person. Whether that counts as the
  member asking the person isn't decided.
- **Full approvals** (7b). In Full, nothing asks before a write outside an agent's folders. Whether Full
  should still protect the person's own checkout isn't decided.
- **Where a hand-over shows** (1e). In the chat the person started, as written here, or in a chat of
  the specialist's.
- **Tools that answer for the person** (12c). Agents hold tools that can answer an approval or a
  question when the person asks them to. A guest gets none of them; what an agent may do with them
  isn't decided.
