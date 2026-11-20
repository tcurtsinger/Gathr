# 0065: Questions are in the lead's words, and an answer reaches the member at once

Status: accepted (owner, 2026-11-14, with Stage 3's spec, part F; the owner's answers of 2026-11-16: "Leave it" for merging, "Leave my list", "A switch", and "Discuss" for the word; its detail and its departures, 2026-11-17, "Approve F"). Date: 2026-11-17.

Built: yes. Proof: `crates/gathr/tests/questions.rs`, `crates/gathr/tests/questions_flow.rs`, `ui/src/tasks/Question.test.tsx`, `ui/src/tasks/TasksTab.test.tsx`.

Builds on [0010](0010-coordination-as-state.md) and [0062](0062-a-plan-is-checked-and-approve-hands-it-out.md). Amends 0010, below. Serves promise 4 of [the charter](../charter.md#the-promises), parts 4.2 to 4.5 of [the truth map](../truth-map.md). Stage 3's part F.

A **question** is a person's task (`kind = 'person'`) an agent gave. A **request** is a member's `person_request`, owned by the chat's Supervisor (the lead). A question is **filed for** a request when its `request` names it: one question, one request, the link the schema already has. A request is **folded** when the lead reports it `blocked` with `on_task` an open question it didn't file for it. **Discuss** is the person's choice to send the lead a message about a question instead of answering it. In examples Mira is the lead, Ann and Bo are members.

## The choice

- **F1. No forward-unchanged.** When the lead asks the person for a request, `what` is required: without it `ask_person` is refused with `` Write `what` in your own words: request #<n> isn't sent to the person as it is ``. The copy of the request's `why`, `choices` and `form` is gone. The request's files are still copied when the lead names none, as it can't reach a member's worktree. The request's brief tells the lead to use `ask_person` with `for_task` and its own `what`, its pick first, to put the request on an open question of its own with `finish_task blocked on_task`, or to answer it itself with `finish_task done`. A request's brief ends with its own line, `If you ask the person, their answer reaches <member> by itself: after ask_person, end your turn with no finish_task.`
- **F2. A filed request waits by itself.** `finish_task blocked` on a request that has a question filed for it still open (`ready` or `held`) is refused: `#<r> waits on #<q>, the person's question, and its answer reaches <member> by itself. End your turn with no finish_task.` A second member's request on the same decision waits by the existing fold, now quiet: a request blocked on an open question is not settled, so its member is not told "blocked" and no turn starts for it. When the question is answered, the fold's carry holds the answer and tells the lead to finish the request with it: `the task blocking this one, #<q>, is done: Picked: …. Finish #<r> done with this answer in note: <member> hears it then.` A cancelled question cancels the folded request with it, and its member hears that, as today.
- **F3. Answered once, delivered at once.** The person's answer settles the request it was filed for in the same transaction: the request is `done` with the reason `the person answered #<q> (<what>): <answer>`, its waits end, a task blocked on it is readied, and its member's hear-back is queued, with no turn of the lead's in between. The lead hears too, in its answered turn, on no task. A settled request is due at once, not after the member's other pending requests (`hear_backs`). Event `gathr.task.request`, `how` `answered`.
- **F4. The pick and the words, apart.** Agents hear `Picked: Connection lost. Their words: and say we retry` (the pick, yes or no, or done; then the words), with ` (answered by <name> for the person)` when an agent answered for them. The question's own `reason`, which the window and the plan's tab show, stays the merged line. A member's own task blocked on its request hears the answer in the readied brief, and the request counts as heard, so no second hear-back follows.
- **F5. Discuss.** An answer may carry `discuss: true` with the person's words and nothing else. The question goes to `held`, leaves the person's list, "needs you" and the chat's dot, and nothing that waits on it starts. The words reach the lead only, in its answered turn under `The person wants to discuss:`, and the question's `answer` keeps `discussed: [{text, turn}]`. The question comes back on the list as it was, when that turn ends (`done`, `failed` or `stopped`), and after a restart for a turn that is no longer to come. A new question in that turn, from the same lead for the same request, takes the old one's place: the old one is cancelled with `#<new> took its place`, tells nobody, and what was folded on it moves to the new one. The person may Discuss again once it is back, and a later answer keeps every entry. A plan stays held while its question is discussed. An agent can't Discuss for the person (`answer_person` never sets it). Events `gathr.task.discussed`, `how` `asked`, `back` or `replaced`.
- **F6. The asking task stays held** until the answer, and hears it then. Nothing was built: the hold the task already has on what its turn gave does it. A test now proves it.
- **F7. The lead's answered turn** says answers and messages apart: `The person answered:` with a line each, `The person wants to discuss:` with a line each, then for a question filed for a request, `#<q> was asked for <member> (#<r>): <member> has the answer.` or `…, who keeps waiting.`, then for a discussed one that it is off the list until the turn ends, to reply there, and that a new `ask_person` takes its place. `Continue.` closes only a turn that carries an answer. The instruction to pass the answer on is gone.
- **F8. Files from every folder the agent writes** (its worktree, its own folder, Shared), a relative path read from the worktree; the refusal is `<path> isn't a file in your folders`. The tool's schema carries the limit up front: `files` at most 5 (`maxItems`), each up to 10 MB.
- **F9. The window.** The first choice of a lead's choice question reads `<lead>'s pick`, then ` · ` and its detail (`An agent's pick` for a giver the window can't name); Gathr's own question has none. Under the answer of a question an agent gave, a row `Discuss` with the shared switch: on, the choices hide and one required field, `Your message`, takes the words; off again, what was picked is still picked. Gathr's own question has no switch. In the chat, at the lead's turn that got the message, a line `You to Mira: <words>` that opens to the question and its task; it stays at that turn when the question is later answered elsewhere. A person's question has no "No report yet" in its task view.
- **F10. Diagnosable.** `gathr.task.answer` (a refused Discuss, with its why and reference), `gathr.task.discussed`, `gathr.task.request`, and `gathr.task.answered` and `gathr.task.state` as today: `gathr diag query "SELECT attrs ->> '$.\"gathr.discuss.how\"' AS how, count(*) FROM event_log WHERE name = 'gathr.task.discussed' GROUP BY 1"`. No migration: a Discuss lives in the existing `answer` column and the state `held`.

## Why

- A member's question forwarded as it was written is a second voice to the person. In the room's record 3 of 4 filed questions were the member's words exactly.
- An answer that reaches the member in the transaction it is given in saves a turn of the lead's, and a lead that forgets to pass it on can no longer leave a member waiting.
- `Picked` and `Their words` keep a choice from being read as a message, and a message from being read as a choice.
- Discuss answers "I need to talk about it first" with the one control the person already uses, and the question never goes through a state that would ready what waits on it or free a plan.
- The question coming back by itself when the lead's turn ends means nothing rests on the lead remembering.

## What was turned down

- **Merging two members' questions now** (the owner, "Leave it"). The record shows no two requests on one decision. It comes back as its own small PR when it first does. Scenario 4d is not added.
- A question as its own table, with its own column and migration.
- Forwarding a request unchanged.
- A discussed question kept on the list and answerable (the owner, "Leave my list").
- A button or a choice row for Discuss (the owner, "A switch").
- Closing a discussed question as done: it would ready what waits on it and free a plan version.
- Keeping a discussed question off the list until the lead remembers to bring it back.
- Refusing a `what` equal to the request's, or scoring how alike they are: no check of words refuses a copy with one word changed and still takes a member's question that was already right, and any threshold would be a guess.
- Skipping the lead's answered turn: on a planned ask the lead writes its plan from it.

## What it amends in 0010

In the bullet on the person's tasks, "The person's answer goes to the Supervisor, who passes it back to the member" becomes "The person's answer settles the request it was filed for in the same transaction, and its member hears it; the Supervisor hears too". "Folds it into one it's already asking" stays, adding "and its member waits for that answer without being told it's blocked". It gains: "The person can Discuss a question instead: it leaves their list and their message goes to the Supervisor; it comes back when that turn ends, unless the Supervisor asks a new question in its place".

## Said plainly

- **A near-duplicate question when two members ask the same thing.** If the lead asks the person twice, they see two questions. If it folds the second, that member hears one lead turn after the first.
- **The lead may still copy the member's words into its question,** or change one word. Counted, not refused. Truth map 4.3 stays Partly.
- **A discussed question is off the person's list for as long as the lead's turn is queued or running.** If that turn waits behind a pause or a usage limit, so does the question. An update or Stop now ends the turn early, and the question comes back before the lead's carry-on replies; a question the lead then asks on a planned ask is refused until it cancels the old one.
- **A new question in that turn about something else, for the same request, replaces the discussed one.** The turn is the lead's reply to it, so that is what such a question is taken to be.
- **A member whose task was cancelled still hears the answer** (one turn, on no task). A member whose lead's turn on its request failed hears "failed" first, as today, then the answer when it comes.
- **A folded request waits up to the stale days** (3 to mark, 10 to cancel by default): if its question stays unanswered that long it is cancelled and its member told.
- **A folded request is cancelled when its question is cancelled,** and its member hears that, as today.
- **A question filed with no `for_task` from a turn on a request is filed for that request,** as today.
- **The plan's tab lists a replaced question with no answer.**
- **A second member on the same decision hears one lead turn later,** and an answer still reaches only a turn that has ended (the ledger's G13): truth map 4.4 stays Partly.
- **Members' tool approvals still come straight to the person,** and nothing checks that the lead asks with options and its pick beyond two choices. Both of AGENTS.md's by-hand rules about the owner's questions stay.
- **Not run by the builders:** macOS, WebKit, the window against a live service, screenshots, and whether a real lead writes its own words and replies well to a Discuss (part K measures it).

## As built

What differs from the detail, from the code as it landed:

- **The Discuss tests are a module.** `tests/questions.rs` alone came to over 1,000 lines, so the Discuss tests sit in `tests/questions/discuss.rs`, a `#[path]` module of it. Several numbered tests are split into named ones. Tests 1, 3 and 7 assert their texts as literals (Ann's request is #3 there, #2 in the kept samples), and the samples `request_brief: choice`, `answered: a member's request` and `answered: discussed` are asserted in `tests/tasks/words.rs`, where their numbers fit.
- **The flows don't add Bo,** and a failing flow test lets its held replay engines go (a panicking-only `Drop` on its room), as a failing test killed only `gathrd`.
- **A discussed question also appears in an answer's `answered` list** (the API's response to a Send), in state `held`.
- **`cancel`'s branch for a request already answered stays.** For new data it can't be reached, since the answer already settles the request; it still covers one answered before the upgrade.
- **Existing tests changed beyond the detail's list:** `Ship it today?: Yes` became `…: Picked: Yes` in `person_tasks.rs` and `coordination.rs` (the new words).
- **Size.** About 350 lines of service code net (+423, −71) against the detail's 215, about 1.6 times; tests about 1,350 new lines. `links.rs` is 221 lines, and `heard_words` and the shared `picked` helper grew. No setting, column, table or migration.
- **Window.** The chat's one `flatMap` draws the answer line and the Discuss lines of each person task, the answer first when one turn carries both. With Discuss on, the words are kept in the draft's notes, so words typed before turning it off show under `Notes (optional)` and go with a pick as its text. The label is a plain span with no helper text.
- **The restart sweep returns a held question with no turn too:** `recover` selects the questions, so one whose `heard_in` is missing comes back on the list rather than failing the start.
- **The request's closing line is conditional:** `If you ask the person, their answer reaches <member> by itself: …`, not the detail's `The person's answer reaches <member> by itself: …`. The same brief opens the lead's turn on a folded request once its question is answered, where the lead must finish it, and the unconditional line told it not to (seen twice in real use).
- **A folded request's carry says what to do:** when the task readied by a question's answer is a `person_request`, its carry ends `Finish #<r> done with this answer in note: <member> hears it then.` A member's own task blocked on its request gets no such sentence.
- **Window places that would show a discussed (`held`) question wrongly, read and not changed:** its task view shows state `held`, an enabled Cancel, and a disabled Restart; and the window drops its staged picks when it goes (the question returns unstaged).

## Amended: answered in place, one at a time (2026-11-17)

The owner approved the Tasks views (the UI pass's `tasks` mock-up, New), with two calls: with a Notes field a pick
marks the choice and Send sends it, with none a pick answers at once; and each question is answered by itself.
This replaces F9's window and the pager described above:

- **One question block.** A question opens as itself in the Tasks tab, from its row or from Open task on its line
  in the chat: the asker's message with the block in its bubble, answered there. The block is one shared component
  (`ui/src/tasks/Question.tsx`), so the chat can answer in it too. There is no pager and no Send of several
  answers: each Send answers one question (`POST /api/person-tasks/answer` with one answer).
- **A pick.** Gathr's own question has no field and no switch, so a pick answers it at once. An agent's has a field
  (Notes, or Answer for a question in words), so a pick marks the choice, with a mark on every choice, and Send or
  Enter sends it. Notes alone still answer it in words.
- **Discuss** sits right under the question, so nothing above it moves when it is turned on. Its field is
  `Message to <asker>`, kept apart from the answer's notes: each is kept while the other shows, and what was picked
  stays picked. This replaces As built's "the words are kept in the draft's notes".
- **A refused Send** stays in the block, under its field: `Not sent`, why, and its reference, with what was typed
  kept; Send tries again. The notice titled "Answer failed:" is gone.
- **Turned down:** the pager kept for questions alone (the owner: each by itself), and a Send that waits for a
  pick on Gathr's question.

## Amended: talked over by Reply in the chat (2026-11-18, 0076)

In the chat, a question waiting on the person is talked over by replying in its thread: the box at the thread's foot
sends the Discuss through the same route (F5), and the question stays open and comes back as F5 says. The chat's
question block has no Discuss switch; the Tasks view keeps it, as above. Gathr's own question offers no Reply, as it
has no switch.
