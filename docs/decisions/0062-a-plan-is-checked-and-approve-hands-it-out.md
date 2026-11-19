# 0062: A plan is a checked record, and Approve hands it out

Status: accepted (owner, 2026-11-14, with Stage 3's spec, part H; its detail and its departures, 2026-11-15, "Approve H"; the answer on a message sent while a plan waits, "Yes, the plan's"). Date: 2026-11-15.

Built: yes. Both of part H's PRs are built. The first: the record, the `plan` tool, the check, Approve, Send back and Drop, and a plain tab. The second: the tab's Findings, Answers and Questions, parts that open one at a time, the marks Changed, New and Removed with "Was", the plan in the needs dock above the composer, and the tab's attention dot. Proof: `crates/gathr/tests/plans/write.rs`, `crates/gathr/tests/plans/check.rs`, `crates/gathr/tests/plans/decide.rs`, `crates/gathr/tests/plan_flow.rs`, `crates/gathr/tests/archive.rs`, `crates/gathr/tests/words.rs`, `ui/src/plans/plans.test.ts`, `ui/src/plans/PlanTab.test.tsx`, `ui/src/chat/needs.test.tsx`.

Builds on [0010](0010-coordination-as-state.md), [0053](0053-a-review-asks-both-questions.md), [0057](0057-agreeing-is-a-reaction.md), [0059](0059-a-work-task-carries-a-complete-brief.md), [0060](0060-gathr-gives-the-review-task.md) and [0061](0061-every-ask-has-a-size.md). Amends 0010 and 0061, below. Serves promise 2 of [the charter](../charter.md#the-promises), parts 2.1, 2.3, 2.5 and 3.5 of [the truth map](../truth-map.md). Stage 3's part H.

Until now "plan" meant Summary's list of the round (`ui/src/chat/plan.ts`). The new names are the module `gathr::plans`, the tables `plans`, `plan_versions` and `plan_parts`, and the column `tasks.checks`. Summary's list is now "Round". In the window an ask's sizes stay "Fix" and "Plan"; `repair` and `new` stay in the code and to agents.

## The choice

- **H1: a plan is a record, and its state is read.** A plan is a row of `plans` (its chat, its ask, its lead) with versions (`plan_versions`, numbered from 1) and each version's parts (`plan_parts`). There is no file. The state comes from the newest version and its check task, never stored apart: `approved` or `dropped` when the person said so; `revising` after a send-back, or while the check has left findings; `waiting` on the person once the version has reached them; `checking` otherwise. An older version is replaced. What is stored is the version's text and parts, when it reached the person (`sent_ts`), the person's word and words, and the ids of Gathr's lines, so the window draws them from the record and never reads Gathr's words back. `plans::state` reads one plan and `plans::open_state` the open plan of an ask, which part I words its refusal by.
- **H2: the `plan` tool.** The chat's Supervisor writes a version with a title, text and parts; each part is a complete task as `give_task` gives one (`to`, `what`, `why`, `done`, `proof`, `where`, `size`, `lines`, `after`, `after_tasks`). Calling it again writes the next version, with `answers` to each finding of the last check. A part is checked by the same function as a task's brief, so there is one set of rules. It is refused, in order and with a reference, for an archived chat, a chat of one agent, a caller who isn't the Supervisor, no turn running, a turn that serves no ask, a repair, a question of the lead's to the person that is still open, a bad title, no parts, a part that isn't whole or names an agent who isn't there or comes after a later part or a cancelled task, two parts with one `what`, findings not answered (or answers with none to answer), and a version the same as the one before. Each version is the lead's own message in its turn, whose body is the whole plan, so `read_chat` reads it and a thumbs-up can go on it. The open plan of the turn's ask gets the next version; with none, a new plan is made.
- **H3: one question at a time.** While a plan for an ask isn't approved, the lead's questions to the person are held to one open at a time, and `plan` waits for it. Gathr's own questions (about a PR) aren't counted.
- **H4: the check.** In the version's transaction Gathr gives a check task, a review with `tasks.checks` set, not a new kind: giver the lead, owner the checker, brief written by Gathr in a new kind of words, `plan_check` (0056). The checker is the one who objected to the version before, else the chat's Reviewer, else the free member that has checked least. A thumbs-up on the version's message by its checker finishes the check, with no second call. A check that objects is `finish_task` left with at least one `wrong` or `unasked` note; the lead hears at once, and answers every finding in its next version, which goes to the same checker with a brief that lists only those findings and the answers. A report of a check takes no PR, commit or `signed_off`. Only Gathr and the person end a check: an agent's cancel, anyone's restart and any edit of one are refused. A done, failed, stopped or cancelled check wakes nobody.
- **H4a: when the check ends,** Gathr says one line in no turn ("Bo agrees with version 1 of the plan", "Bo has 3 findings on it", "Bo didn't finish checking it") and, but for findings, the version has reached the person. A check still open 10 minutes after the version was written reaches them as "didn't finish": that rides the quiet look (so 10 to 12 minutes), with no new timer, and `GATHR_PLAN_CHECK_MS` shortens it in dev and tests only.
- **H5: the person is told by what exists.** A version waiting on the person is a line in what the chat waits on ("A plan for you: <title>"), a reason for the chat's needs-you mark and for B's notification ([0058](0058-gathr-tells-you-outside-the-window.md)). The window draws the version as its own block with a chip, "Plan · version 2", the part count and the thumbs-up under it; Gathr's lines (how the check ended, Approved, Dropped, the send-back) as one line of a kind of their own, apart from the size line; and the plan in a tab beside Tasks and the agent's, with the title, the ask, the text, the check with its findings, the lead's answers and the ask's questions, and each part as a line that opens to its brief, marked Changed, New or Removed against the version before, and a foot with Drop, Send back and Approve. A plan waiting on the person is also in the needs dock above the composer, with Open (since 2026-11-18 the status line's "needs you" opens it instead: 0037), and its tab carries the attention dot.
- **H6: Approve makes the tasks.** In one transaction, each part of the newest version becomes a work task, word for word, given by the plan's lead in the version's turn, after the parts and tasks it names. The version's open check is cancelled after the version is marked approved, so no check line follows. No turn of the lead's is spent: it hears back on the tasks as on any it gave, and learns of Approve from the line. Approve and Send back wait until the version has reached the person, and refuse a plan whose lead no longer leads, a part whose owner has left the chat and a part that comes after a cancelled task. **Send back** asks for words and queues a turn for the lead in Gathr's own words (`sent_back`), answering the version's turn. **Drop** needs no confirm: it loses nothing, and the lead can write a new plan.
- **H7: a second plan.** `plan` for an ask whose plans are all approved or dropped makes a new plan; the first keeps its record. Approve gives tasks only for its own parts.
- **H8: where plans touch what G built.** An open plan (checked, waiting or being revised) is open work of its ask, so a person's message while it is open joins its ask ([0061](0061-every-ask-has-a-size.md)). A split never moves a plan, its check or the tasks Approve made: the split is refused when the message's turn wrote a version, and neither task update moves a check or a part's task. A person's resize of a planned ask to repair changes nothing of the plan.
- **The API.** `GET /api/chats/<id>/tasks` and the live `tasks` event carry `plans`, each task carries `checks` and `plan`; `GET /api/plans/<plan>` gives a plan whole; `POST /api/plans/<plan>/approve`, `send-back` and `drop` ([api-tasks.md](../api-tasks.md)). The events are `gathr.plan.version`, `gathr.plan.checked` and `gathr.plan.decided`. No new live event.
- **The schema** (migration `0047_plans.sql`): the three tables, `tasks.checks` and its index. No foreign key from a version to its message or turn: deleting a chat removes messages and turns before its plans, and `archive::delete` removes the plans after its tasks.

## Why

- A plan the person approves has to be one thing Gathr holds, or the lead's words and the tasks made from them can drift. Read from rows, its state can't disagree with them.
- The check is a review task so the places that already treat a review (reports, hear-backs, the waiting lines, the quiet look) work for it; a fifth kind would rebuild `tasks`. The places that differ for a check are each listed in the code and turned off by one test: `tasks.checks`.
- An objection must name something wrong or unasked, or a left report would send the lead a list with nothing to answer.
- A message of the person's while a plan waits has to land on the plan's ask, or the next version would start a second plan and Approve would hand out tasks under the wrong ask.

## Different from the spec

The owner approved each with the detail on 2026-11-15.

- **States are read, not stored.** The spec lists eight; "drafting" has no row because a plan exists from its first version, "replaced" is a version's, and "sent back" reads as revising.
- **No read tool.** The version is a message, and `read_chat` reads it whole.
- **The check is a review with `tasks.checks`,** not a kind. Only Gathr and the person end a check; nobody restarts or edits one.
- **The lead answers every finding in its next version.** The spec says "a new version or a reason": a reason rides on a version as an answer with `changed: false`, and the same checker checks it.
- **A check that objects must name a wrong or unasked note;** suggestions alone go with a thumbs-up or a done report.
- **A reaction settles the check at the end of the checker's turn,** as any report does. The spec's "no second call" holds.
- **Ten minutes ride the quiet look:** 10 to 12 minutes from when the version was written.
- **The lead isn't woken when a check agrees, fails or runs late.**
- **Approve and Send back wait for the check.** Drop doesn't.
- **Parts are matched by `what`, in the window only:** nothing on the service stores a part id or a diff. The match is on the trimmed `what`, so a part moved in order with the same fields reads Unchanged.
- **The words are shorter than the mock-up's:** the chip is the line "sent a plan"; a check reads "Plan v1: 3 findings"; the foot has no sentence that explains a button.
- **A plan's lead that no longer leads the chat** can't have its plan approved or sent back.
- **A split that would move a plan is refused,** and a split never moves a plan's tasks.
- **The first PR is larger than the spec's 900 + 500 lines:** its service code and tests are the spec's own first PR and couldn't be cut.

## As built

Where the code differs from the detail the owner approved, the code stands.

- **The `again` form of a check's brief** is used only when the version before has a check that left wrong or unasked notes, that version is undecided and the new check's owner is the one who left them. After a send-back the first form is used.
- **A plan's check in `GET /api/plans`** has an `outcome` of `agrees`, `findings` or `unfinished` for a settled check (a check that left findings keeps `findings` when a newer version or the person cancels it), `late` for an unsettled check whose version was already sent, and null otherwise.
- **Send back cancels the version's check first,** so a check turn that was running stops with the reason "version 1 was sent back", and not "version 2 replaced it". The next version's check goes to the one who objected, as the detail says.
- **Plurals:** "1 part", "1 task", "1 finding".
- **The window's marks** (`marks` in `ui/src/plans/plans.ts`) compare each part with the part of the version before that has the same trimmed `what`: none reads New; a different owner, Why, Done when, Proof, Where, Size (its words or its lines) or After reads Changed; else Unchanged, with no mark drawn. After is compared by the `what` of the parts it names and by the task numbers, so a renumbering isn't a change. A part of the version before with no match is Removed, so a renamed part reads Removed and New.
- **A part opens to its brief,** one at a time, and none is open when the tab first draws. The rows are Why, Done when, Proof, Where, Size and After (`briefRows` in `ui/src/tasks/model.ts`, shared with the task's page, which has never shown Why). Window, 2026-11-18 (the plan-tab mock-up, New, order A): each part is the Tasks list's one row (`Row` in `ui/src/tasks/TaskList.tsx`), "Part 2 · Ann · After 1", with Changed or New as a word at the end of its line; an open part is its row and its brief on one tint. A changed field's old words sit under its new ones as "Was: ...", in the field's own row; a changed owner has an Owner row; After's Was names each part by its number in the version shown, or by its what with ", removed" when it went, never by the older version's number. A removed part is an ended row, its what struck through, "Ann · Removed". Once approved, a part whose task the chat has is that task's row and opens it in Tasks; a part with no task keeps its part row. The tab's state is a word under the title, Waiting included; Asked is a field, and a sent-back version shows the person's words as Sent back. The tab opens at its top when the version shown changes and when the person's own Send back, Approve or Drop goes through; a state that changes from elsewhere, such as a check that ends, keeps the place. The foot shows only when something in it acts: Drop, Send back and Approve, and a refusal over them ("Not approved", why, its reference) in place of the failure notice at the top; "Saving" is gone, the buttons are off while a request runs. Send back's field is the question block's field (`Field` in `ui/src/tasks/Question.tsx`, without its send arrow), so it reads like the answer form, and the inline elicitation form left the registry with its last user.
- **Findings and Answers use the format of a task's notes** (`noteLine`, shared with `noteLines`), so a finding reads "Wrong · src/notes.rs: ...", not "Wrong: src/notes.rs: ...": one copy of the format. Findings leave out suggestions. An answer is its finding's line, then "Changed: ..." or "Kept: ..."; an answer whose note isn't in the version before's check reads "Note 12". Questions are the ask's, each with its answer or "Open".
- **The tab's dot** is driven by `PlanTab`, which tells the app (`onWaits`) whether its plan waits on the person, and follows live `tasks` events; the workspace draws the attention mark on the tab as it does on Tasks. The detail doesn't say how.
- **The dock** (removed 2026-11-18: the status line's "needs you" opens the plan instead, 0037; later that day the
  line went too, and a needs-you row in Summary opens it, 0050) showed the oldest approval first, else the oldest plan waiting on the person, as "Needs you · 1 of N" with "<lead>'s plan: <title> · 9 parts" and Open, which opens the plan's tab at its newest version. N counts approvals and plans waiting; a plan being checked or approved isn't counted. The lead's name comes from the chat's members.
- **The window reads a check's outcome from the record,** not from Gathr's words: findings when the version has no `sent`; "agrees" only for a done check that reported no later than the line; else "didn't finish". The tab reads the service's `outcome`, so after a late agree the tab says "Bo agrees" while the line stays "didn't finish".
- **The send-back line's words** are read from the send-back message's body, as the tasks list carries no `words`; `GET /api/plans` does.
- **The versions links** (`v1 · v2`) show only when the plan has more than one version. Send back's field shows only on the newest waiting version.
- **The chip's styles** are `ui/src/plans/plans.css`; a version's custom message carries `{plan, version, parts}`, with the speaker and the thumbs-up where a reply carries them.
- **`gathr::team::agent_named`** is the one place a part's owner is found, used by the `plan` tool and `give_task`.

## Said plainly

- **No cap on rounds of findings.** A lead and a checker can go version after version: each left check wakes the lead, and each new version goes to the same checker. The record counts it: `gathr diag query "SELECT v.plan_id AS plan, v.n AS version, count(r.id) AS findings FROM plan_versions v LEFT JOIN tasks t ON t.checks = v.id LEFT JOIN review_notes r ON r.task = t.number AND r.kind IN ('wrong', 'unasked') GROUP BY v.id ORDER BY v.plan_id, v.n" --data <folder>`. Nothing stops it until the record shows it happening.
- **A late version can be approved unchecked.** It shows "didn't finish", and Approve takes it and cancels the still-running check, so findings it would have made are never recorded. The person saw it and chose. The share of late checks: `gathr diag query "SELECT count(*) AS checked, sum(attrs ->> '$.\"gathr.plan.outcome\"' = 'late') AS late FROM event_log WHERE name = 'gathr.plan.checked' AND status = 'ok'" --data <folder>`. The room's record of PR reviews, the nearest there is, has about 7% of first review turns ending after 10 minutes.
- **A waiting plan catches the person's messages.** Until it is approved, sent back and rewritten, or dropped, every message the person sends while no newer ask has open work joins the plan's ask, even on a new subject, and the lead must split each one off. The owner took this cost with the answer "Yes, the plan's".
- **A left check reads as any blocked task does** in the foot: "#12 (Bo) is blocked: <the findings> Mira hears of it next", then "Mira was told". While the lead's revising turn runs, its own line shows and the chat isn't stuck.
- **A plan left `revising` has no timer.** A left check is settled, so the 10 minutes don't apply; a lead that writes no next version leaves the plan waiting on it, and only the quiet look tells the person.
- **A removed checker's check runs on:** removing an agent leaves its tasks and turns as they are, and its thumbs-up still settles the check. If it never ends, the version still reaches the person at 10 minutes.
- **Approve's tasks join Summary's round of the version's turn,** so they show in its current round unless the lead has given tasks in a newer one since.
- **Not verified:** that rmcp's schema for the nested `parts` reaches real engines as written (only the stand-ins' tool list was read); that the live message for a version can arrive before the `tasks` event that names it as a plan (until then it draws as plain text of the lead); that a checker deleted after its removal ends its check task as a normal turn would; how well real agents agree by thumbs-up, write findings with evidence, or answer them (part K). The window was not looked at rendered in light and dark at 480 and 1024 wide when this was written, the second PR's marks, Was, Removed rows, tab dot and dock included; no test draws the dot on the tab in the workspace.

## Turned down

- **A plan file.**
- **A read tool for plans,** since the version is a message.
- **A new task kind for the check.**
- **Storing every state,** which could disagree with its rows.
- **A stored diff or a stored part id:** the window matches parts by `what`.
- **Parts matched by a number the lead gives.**
- **A reason with no version.**
- **Agents cancelling, restarting or editing a check.**
- **A confirm on Drop.**
- **The person's message for Send back,** which would start or join an ask and could land on another ask's open work.
- **A new live event** (the `tasks` event carries `plans`).
- **A new timer for the 10 minutes.**
- **Every free member checking.**
- **A setting for any of it.**
- **The mock-up's explaining words** ("starts when you approve", "9 tasks · 2 start now") and its Home tab.

## Amends

- [0010](0010-coordination-as-state.md)'s Plans and plan checks: a plan is a record with versions and no file; its states are read; the check is a review task with `tasks.checks`, not a `plan_check` kind; the checker is the one who objected, else the Reviewer, else the free member that checked least; the lead answers every finding in its next version.
- [0061](0061-every-ask-has-a-size.md): an open plan is open work of its ask, and a split never moves a plan, its check or the tasks Approve made.

## Amended by

- [0063](0063-building-waits-for-a-new-asks-plan.md): Approve releases the ask's tasks that wait for its plan.
- [0075](0075-a-team-chat-is-main-messages-each-with-its-thread.md): each version is a main message of its own, with
  its state at its name line's right (Checking, Needs you, Working, Approved, Sent back, Dropped); its check, Gathr's
  lines about it and the lead's turn a send-back started sit in its thread. Since 2026-11-18 (the owner: "Remove all
  of it") a version says no state there; the plan's tab says it.
- [0078](0078-said-to-you-is-main-one-bubble-per-turn.md): a version is a chip in the bubble of the lead's turn that wrote it, where it wrote it, and no message of
  its own; its check and Gathr's lines about it sit in the thread of that bubble, and the turn a send-back started is the
  lead's next main message, quoting "Sent back" and the person's words. While it waits, the bubble's like approves it.
  Since 2026-11-19 the check's findings line ("Plan v1: 3 findings") is no longer drawn: a chip at the end of the
  checker's bubble says "3 findings" and opens to each note; a note's kind `unasked` reads "Not asked for", in the
  plan's tab and the Tasks view too, drawn by one component (`ui/src/tasks/Note.tsx`) in place of `noteLine`.
