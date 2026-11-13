# The truth map

Each promise in [the charter](charter.md#the-promises), part by part, against the code as it is today.
Read a promise's rows before planning work on it. A PR that builds or changes a part changes its row in
the same PR.

The states are the charter's words:

- **Written:** a document says it. Nothing in the code does it.
- **Partly:** some of it is in the code. The last column says which part is missing. A part with code
  and no test that would fail is Partly too, and says so.
- **Built:** the code does it, and the test named fails if it stops.
- **Kept:** built, and its number has held in real use for three days. Nothing is Kept yet: no number has
  been counted for three days.

The Code column names the file and what in it does the part. The Test column names a test as
`file::name`. `node scripts/docs.check.mjs` fails when a file named here doesn't exist or a test named
here isn't in its file. It can't tell whether the words are true: each row was read against the code on
2026-11-12, at `main` b1453e7, and nothing was run to write it. Every row that says Built was then read a second time, test body and all, by a reviewer that hadn't written it. "Against" marks code that does the
opposite of what the promise says.

What each decision still lacks is in [the decisions list](decisions/README.md). What each part must do,
as a test will see it, is in [the scenarios](scenarios.md).

## 1. An assistant by default, your own specialists, and a team when the work needs one

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 1.1 | A new chat with no agent named is with your assistant, which does small things itself | Written | | | No assistant exists. A new chat starts with the first agent listed already picked (`ui/src/chat/ChatView.tsx`) |
| 1.2 | In a new chat, @ an agent and you talk to it | Partly | `ui/src/chat/AgentPicker.tsx` `AgentPicker` | `ui/src/chat/ChatView.test.tsx::a new chat is made with its first message, in its project, to its Supervisor` | You pick the agent with a button, the one the charter says there shouldn't be. Typing @ does nothing in a new chat |
| 1.3 | In a team chat, @Name gives the message to that member | Partly | `crates/gathr/src/team.rs` `addressee` | `crates/gathr/tests/team.rs::a_message_goes_to_the_supervisor_unless_it_names_a_member` | Over the API only. Against: the window always sends the agent its picker shows, and that agent wins over an @Name in the text (read from the code, not tried in the app) |
| 1.4 | You create standing agents, each with a name, an account, a model and an effort | Built | `crates/gathr/src/agents.rs` `add`, `change` | `crates/gathr/tests/agents.rs::a_turn_runs_on_its_agents_account_model_and_effort_and_a_change_applies_from_the_next` | |
| 1.5 | An agent has a job description, which is how others know what it is for | Written | | | Agents have no such field. The briefing gives teammates' names and roles only |
| 1.6 | The assistant hands an ask to the specialist it belongs to | Written | | | No agent can pass an ask to an agent outside its chat |
| 1.7 | For a project, a team is proposed from your agents and agreed before planning | Written | | | A project's team is set by hand (`ui/src/NewProject.tsx`). Nothing proposes one |
| 1.8 | A team is five agents at most, with one lead and a reviewer | Partly | `crates/gathr/src/team.rs` `refused` | `crates/gathr/tests/team.rs::role_rules_are_refused_through_the_api` | The limit of five, one lead once there are two, and at most one reviewer are held. A reviewer isn't required: a team of a lead and a member is accepted |
| 1.9 | A chat sits under its agent's name, inside its project | Partly | `ui/src/Sidebar.tsx` `Sidebar` | `ui/src/live-lists.test.tsx::a project and chat made elsewhere show in the sidebar, and the chat opens when none was` | Chats sit under their project. Nothing groups them by agent. The test proves both show, not where |
| 1.10 | An agent's know-how grows: about you, about a project, about its craft | Written | | | No memory and no skills. Against: the engines' own memory and bundled skills are switched off ([0024](decisions/0024-only-what-gathr-gives.md)) |

## 2. New work is planned with you and checked first. Repairs don't wait for you

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 2.1 | A plan is a record Gathr holds, with versions and states | Written | | | No plan record. The window draws a "plan" from the lead's newest tasks (`ui/src/chat/plan.ts`) |
| 2.2 | Nothing new is handed out before you approve its plan | Written | | | Against: `give` in `crates/gathr/src/tasks.rs` hands a task out at once, from any member, with no plan |
| 2.3 | No plan reaches you before the reviewer has checked it against the code | Written | | | No plan check. Review tasks are for PRs only |
| 2.4 | An ask is sized as everyday, repair or new, and Gathr holds the size | Written | | | Nothing sizes an ask |
| 2.5 | For new work the lead asks its questions first, one at a time | Written | | | A question can be asked (4.2), but nothing ties questions to a plan or to new work |
| 2.6 | A repair goes ahead without asking, and is shown to you after | Written | | | True only because nothing gates any work. Gathr knows no repair, and shows none after |

## 3. Every task stands on its own

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 3.1 | A task carries what to do | Built | `crates/gathr/src/tasks.rs` `give`, `crates/gathr/src/tasks/words.rs` `brief` | `crates/gathr/tests/tasks.rs::a_task_waits_for_its_after_tasks_then_starts_its_owners_turn` | One line of at most 80 characters |
| 3.2 | A task carries why | Partly | `crates/gathr/src/tasks/words.rs` `brief` | | Optional, and no test fails if the brief drops it |
| 3.3 | A task carries what done means and how it's proved | Written | | | Neither field exists |
| 3.4 | A task carries where the work lives | Partly | `crates/gathr/src/tasks.rs` `pr_looked` | `crates/gathr/tests/tasks.rs::a_tasks_pr_settles_it_and_a_review_follows_its_commit` | A task may name its PR. Its branch and folder aren't on it, and the brief doesn't say where to work |
| 3.5 | Approving a plan makes the tasks from its parts | Written | | | No plan (2.1) |
| 3.6 | A task set after another starts only when that one's work is done | Built | `crates/gathr/src/tasks.rs` `may_start`, `waiting_on`, `cascade` | `crates/gathr/tests/tasks.rs::a_task_waits_for_its_after_tasks_then_starts_its_owners_turn`, `crates/gathr/tests/tasks.rs::a_task_done_and_held_only_on_its_pr_lets_the_tasks_after_it_start` | Done, or reported done and held only on its PR's merge |
| 3.7 | An agent with no memory of the chat can do the task right | Written | | | Nothing tries a task from its brief alone. An agent keeps one session per chat, so it has always had the chat |

## 4. One voice to you

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 4.1 | On a team only the lead asks you: a member's ask becomes a request the lead owns | Built | `crates/gathr/src/tasks/person.rs` `ask`, `lead_of` | `crates/gathr/tests/person_tasks.rs::a_members_ask_goes_through_the_supervisor_to_the_person_and_the_answer_comes_back` | For questions. A member's tool approvals still come straight to you |
| 4.2 | A question is a record you answer in the app, with choices and the lead's pick first | Partly | `crates/gathr/src/tasks/person.rs` `ask`, `answer` | `crates/gathr/tests/coordination.rs::only_the_lead_asks_the_person_and_each_answer_is_taken_or_refused_with_a_reference` | The question, its choices and its answer are held. The pick is only the first choice by order, shown as Recommended, and no test holds that |
| 4.3 | The lead asks in its own words | Written | | | Against: the lead is told to "omit `what` to forward it unchanged" (`request_lines` in `crates/gathr/src/tasks/person.rs`), and the test for 4.1 holds that it does |
| 4.4 | An answer reaches everyone who waits on it at once | Partly | `crates/gathr/src/tasks/person.rs` `answer_as` | `crates/gathr/tests/person_tasks.rs::each_answer_in_a_send_is_answered_or_refused_on_its_own` | Only the lead hears at once. The member who asked waits for a turn of the lead's to pass the answer on |
| 4.5 | A question holds the task that asked until it's answered | Partly | `crates/gathr/src/tasks/held.rs` `open_on`, `settle` | `crates/gathr/tests/person_tasks.rs::a_members_ask_goes_through_the_supervisor_to_the_person_and_the_answer_comes_back` | The test holds that the lead's request waits on the person. No test holds that the member's own task stays open meanwhile |

## 5. Work tells the truth

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 5.1 | A task's owner reports it: done, left with what's left, or blocked on what | Built | `crates/gathr/src/tasks/report.rs` `finish` | `crates/gathr/tests/tasks.rs::a_finish_applies_when_the_turn_ends_and_only_the_owner_gives_one`, `crates/gathr/tests/tasks/reports.rs::left_with_nothing_open_stops_and_tells_the_giver` | The report is kept on the task until its owner reports again or the task is restarted ([0047](decisions/0047-a-report-is-a-record.md)) |
| 5.2 | A turn ending is never read as work ending | Built | `crates/gathr/src/tasks/held.rs` `settle`, `released` | `crates/gathr/tests/tasks/reports.rs::a_turn_that_ends_with_no_report_asks_its_owner_once_then_tells_the_giver`, `crates/gathr/tests/tasks/reports.rs::a_hold_that_ends_with_no_report_wakes_its_owner`, `crates/gathr/tests/coordination.rs::a_task_given_by_tool_runs_under_the_turn_that_gave_it_and_its_giver_hears_back`, `crates/gathr/tests/codex_recovery.rs::on_codex_a_turn_that_ends_is_not_a_task_done_and_a_late_report_is_taken` | The last two run it through the service on a Claude Code and a Codex recording |
| 5.3 | "Done" says what to look at | Built | `crates/gathr/src/tasks/report.rs` `finish` | `crates/gathr/tests/tasks/reports.rs::a_report_that_says_too_little_is_turned_away_saying_what_is_missing`, `crates/gathr/tests/coordination.rs::a_task_its_owner_reports_done_is_done_when_its_turn_ends_and_its_giver_hears_so` | Work's done needs a PR, a commit, a file or a note; a review's needs its commit. The task's page shows it (`ui/src/tasks/TaskReport.test.tsx`). Gathr doesn't check that what is named exists |
| 5.4 | A truthful report is never refused | Built | `crates/gathr/src/tasks/report.rs` `finish` | `crates/gathr/tests/tasks/reports.rs::a_report_is_taken_in_every_state_a_task_can_be_in`, `crates/gathr/tests/tasks/commands.rs::its_owner_finishes_a_held_task_from_any_turn` | A review that is done can't be recorded again at a newer commit yet: it answers "Already recorded" |
| 5.5 | A task that waits on its PR's merge is held, and is done when Gathr sees the merge | Built | `crates/gathr/src/tasks.rs` `pr_looked` | `crates/gathr/tests/tasks.rs::a_tasks_pr_settles_it_and_a_review_follows_its_commit`, `crates/gathr/tests/tasks/reports.rs::a_pr_that_merges_does_not_make_a_left_task_done` | Work is done by its merge only if its owner reported it done; else its owner is woken with what it last said (`crates/gathr/tests/tasks/reports.rs::a_pr_that_merges_finishes_work_only_if_its_owner_reported_it_done`). A review is done by the merge either way |
| 5.6 | What Gathr can see holds a task open: a wait, a task handed on, a background command, a question | Built | `crates/gathr/src/tasks/held.rs` `open_on` | `crates/gathr/tests/tasks.rs::a_turn_that_ends_with_work_open_holds_its_task_and_says_on_what`, `crates/gathr/tests/pauses.rs::a_held_task_settles_when_its_command_ends_inside_another_turn`, `crates/gathr/tests/person_tasks.rs::a_members_ask_goes_through_the_supervisor_to_the_person_and_the_answer_comes_back`, `crates/gathr/tests/tasks/hear_backs.rs::a_task_given_in_passing_holds_nothing_and_is_heard_back_on_by_itself` | The first test holds the wait and the task handed on, the second the background command, the third the question. A task given in passing holds nothing (the fourth test) |
| 5.7 | Whoever gave a task hears back once, with the real outcome | Built | `crates/gathr/src/tasks/held.rs` `hear_backs` | `crates/gathr/tests/tasks.rs::the_giver_hears_back_once_its_turn_has_ended_and_every_task_from_it_is_settled`, `crates/gathr/tests/tasks/hear_backs.rs::a_hear_back_names_only_what_its_giver_has_not_heard`, `crates/gathr/tests/tasks/hear_backs.rs::an_outcome_that_changes_after_it_was_heard_is_heard_once_more` | After Stop now the giver hears nothing until the carry-on ends, then that outcome (5.10's test). A hear-back names only what its giver hasn't heard, and an outcome that changes after it was heard is heard once more |
| 5.8 | A review is recorded at the PR's exact commit, and an update that only brings in the base keeps it | Built | `crates/gathr/src/bin/gathrd/merging.rs` `reviewed_is_head`, `crates/gathr/src/bin/gathrd/merging/review.rs` `base_updates_only` | `crates/gathr/tests/merge/review_updates.rs::a_done_review_covers_a_clean_update_from_main` | The merge tool keeps it. Gathr's own look at the PR still marks the review outdated on any new commit |
| 5.9 | An update of Gathr never marks a task under way as stopped, and its agent carries on | Built | `crates/gathr/src/carry.rs` `save_all`, `crates/gathr/src/tasks.rs` `recover` | `crates/gathr/tests/update_room.rs::an_update_in_the_middle_of_a_task_carries_it_on_and_tells_its_lead_nothing_false` | |
| 5.10 | Stop now leaves a task as it was while its carry-on waits for Restart | Built | `crates/gathr/src/tasks.rs` `turn_moved`, `crates/gathr/src/bin/gathrd/turns/holds.rs` `stop_now`, `limited_end` | `crates/gathr/tests/update_room.rs::stop_now_leaves_the_task_as_it_was_and_its_lead_hears_only_how_it_ends`, `crates/gathr/tests/pauses.rs::a_usage_limit_after_a_tool_leaves_the_task_as_it_was` | The task says "Stopped by you. Carries on at Restart". A usage limit that cuts a turn after a tool ran does the same. If the carry-on can't be saved, the task is marked stopped as before |

## 6. The room explains itself

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 6.1 | Gathr can say who is doing what | Built | `crates/gathr/src/bin/gathrd/chat_state.rs` `States`, `ui/src/chat/glance.ts` `glances` | `crates/gathr/tests/chat_state.rs::a_chat_works_from_its_turns_start_to_its_end_and_not_while_its_turn_is_queued`, `ui/src/chat/Summary.test.tsx::the button names who needs the person and who works, and stacks their initials: workers ringed, the dot once` | The service says whether a chat works and whether it needs you. Who it is, is put together in the window |
| 6.2 | Gathr can say what the room is waiting on | Partly | `ui/src/chat/glance.ts` `standing`, `ui/src/chat/waits.ts` `waitingOn` | `ui/src/chat/glance.test.ts::an agent that is not working says why: held or paused, stopped, out of usage, or what it waits for` | In the window, agent by agent. The service has no one answer that lists every hold with its age, and agents can't ask |
| 6.3 | A quiet room is noticed by Gathr, before you notice | Partly | `crates/gathr/src/tasks/stale.rs` `sweep`, `crates/gathr/src/bin/gathrd/turns/holds.rs` `check_stall` | `crates/gathr/tests/stale.rs::a_task_nothing_moves_for_the_stale_days_is_marked_once_and_its_supervisor_told` | The lead is told after three days. A turn silent for ten minutes is flagged as an event, which no screen shows and no agent is told. Nothing checks "work is owed and nothing runs" |
| 6.4 | Agents talk to each other in the open, in the chat | Built | `crates/gathr/src/bin/gathrd/tools/message.rs` `message` | `crates/gathr/tests/messages.rs::a_message_to_an_idle_member_shows_and_wakes_it_once_answering_the_senders_turn` | |
| 6.5 | Their messages read like a thread: an @, with the answer under it | Partly | `ui/src/chat/messages.ts` | `ui/src/chat/Timeline.test.tsx::agents’ messages to each other read as one block, each headed by who wrote to whom` | They show as one block headed "Juno to Orin", sent with a tool. No @, and no answer under a message |
| 6.6 | Agreeing is a reaction, never a message | Written | | | No reaction exists, and nothing stops a message that only agrees |

## 7. Everything in its place

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 7.1 | Each agent in a project chat works in its own worktree, in its own folder, on its own branch | Built | `crates/gathr/src/worktree.rs` `ensure`, `branch`, `crates/gathr/src/bin/gathrd/turns/workspace.rs` | `crates/gathr/tests/team.rs::each_agent_works_in_its_own_worktree_from_the_fetched_default_branch`, `crates/gathr/tests/agent_folder.rs::a_project_chats_worktree_and_temp_files_land_in_the_agents_own_folder` | |
| 7.2 | A file an agent writes outside its own folders asks you first | Partly | `crates/gathr/src/approvals.rs` `ask`, `crates/gathr/src/bin/gathrd/tools/hook.rs` `pre_tool_use` | `crates/gathr/tests/agent_folder.rs::an_agent_writes_in_its_own_folder_and_shared_without_asking_and_is_asked_elsewhere` | Claude Code's file tools ask. In Full approvals nothing asks, on either engine. Codex in Auto asks Codex's own reviewer, not you |
| 7.3 | A shell command's writes outside its folders are stopped or caught | Partly | `crates/gathr/src/codex/rpc.rs` `open_thread` | `crates/gathr/src/codex/rpc.rs::the_agents_folders_open_the_thread_as_writable_roots` | Codex only, by its sandbox. A Claude Code shell command can write anywhere and nothing sees it. The test proves what Gathr asks Codex for, not that Codex holds to it |
| 7.4 | A worktree goes when its chat is archived or deleted, and when its PR has merged or closed | Built | `crates/gathr/src/bin/gathrd/cleanup.rs` `sweep`, `crates/gathr/src/bin/gathrd/cleanup/prs.rs` | `crates/gathr/tests/agent_folder.rs::archiving_removes_a_worktree_once_nothing_runs_in_it_and_nothing_is_left_uncommitted`, `crates/gathr/tests/archive.rs::deleting_a_project_chat_removes_its_rows_and_worktrees_and_keeps_its_branches`, `crates/gathr/tests/pr_cleanup.rs::merged_pr_removes_its_worktree_and_branch_but_keeps_other_branches` | A worktree with uncommitted work is kept, on purpose. On archive and delete the branch stays ([0034](decisions/0034-agent-folder.md)) |
| 7.5 | A short program Gathr starts ends with everything it started | Built | `crates/gathr/src/run.rs` `started` | `crates/gathr/tests/run.rs::a_check_dropped_after_its_shell_exits_ends_what_it_left_running` | |
| 7.6 | What an engine starts ends with its work | Partly | `crates/gathr/src/bin/gathrd/turns/holds.rs` `stop_now`, `crates/gathr/src/claude/engine.rs` `close` | | Claude Code's background commands are asked to stop. Against: closing an engine ends only its own process, and a test shows a child outliving it (`a_killed_engines_children_dont_hold_up_the_hand_over`). Codex has none of this. No test holds the built half |
| 7.7 | Gathr knows the background commands an agent started | Partly | `crates/gathr/src/background.rs` `set`, `of` | `crates/gathr/tests/pauses.rs::a_held_task_settles_when_its_command_ends_inside_another_turn` | Claude Code only |
| 7.8 | A test copy of Gathr can't be mistaken for your own | Partly | `crates/gathr/src/launch.rs` `folder_key` | `crates/gathr/src/launch.rs::each_data_folder_gets_its_own_window_and_the_persons_own_keeps_the_plain_one` | Each data folder gets its own window. Against: an installed window on another folder is titled "Gathr" like yours, and updates itself from the real feed |
| 7.9 | An engine's temporary files go when it closes | Built | `crates/gathr/src/folders.rs` `engine_closed` | `crates/gathr/tests/agent_folder.rs::an_engines_temp_folder_lasts_as_long_as_the_engine` | |

## 8. Context lives in the record

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 8.1 | A task has a record: its brief, its questions and answers, its reviews, its PR and each report | Partly | `crates/gathr/src/tasks.rs` `Task` | `crates/gathr/tests/tasks.rs::a_tasks_pr_settles_it_and_a_review_follows_its_commit` | The brief, the PR and the reviewed commit are kept. A report isn't: only the last reason stays on the task. A review is its own task, joined by the PR's number |
| 8.2 | Every turn on a task opens with that record | Partly | `crates/gathr/src/tasks/words.rs` `brief` | `crates/gathr/tests/tasks.rs::a_task_waits_for_its_after_tasks_then_starts_its_owners_turn` | A task's own turns open with what, why and where it left off. Wake-ups, hear-backs and carry-ons open with their own line only. Questions, answers and reviews are never in it |
| 8.3 | Gathr sees a compaction, compacts when asked, and compacts by itself at a set point | Built | `crates/gathr/src/context.rs` `point_for`, `crates/gathr/src/bin/gathrd/turns/compact.rs` | `crates/gathr/tests/context.rs::a_compaction_says_so_in_the_chat_and_shows_the_fill_after_it`, `crates/gathr/tests/context.rs::compact_asks_codex_to_compact_its_thread_and_says_so`, `crates/gathr/tests/context.rs::claude_code_starts_with_auto_compact_capped_at_80_percent_of_its_window` | |
| 8.4 | After a compaction an agent gets back its role, the plan's state and its open tasks | Written | | | Nothing is given back. Before one, a turn near the point is told to save its notes |
| 8.5 | After Gathr restarts, a turn that was cut off carries on by itself | Partly | `crates/gathr/src/carry.rs` `save_all` | `crates/gathr/tests/update_room.rs::an_update_in_the_middle_of_a_task_carries_it_on_and_tells_its_lead_nothing_false` | After an update. Against: after any other restart or a crash, `recover` in `crates/gathr/src/tasks.rs` marks the task stopped until its giver or you restart it |
| 8.6 | An agent's conversation with its engine is kept and taken up again | Built | `crates/gathr/src/store.rs` `engine_session`, `crates/gathr/src/bin/gathrd/turns/process.rs` `spawn` | `crates/gathr/tests/engines.rs::an_idle_engine_closes_gently_and_the_next_turn_resumes_its_session` | The test closes the engine, not the service |
| 8.7 | An agent is told its team and its role | Partly | `crates/gathr/src/bin/gathrd/tools/briefing.rs` `text` | `crates/gathr/tests/team.rs::a_new_supervisor_reaches_the_next_turns_briefing` | The role is named ("as its Supervisor"). Nothing says what a lead, a reviewer or a member does |
| 8.8 | A new session picks a task up and loses nothing | Written | | | An agent keeps one session per chat. Nothing starts a task in a new one |

## 9. Any engine, same behaviour

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 9.1 | Claude Code and Codex run behind one interface, in any seat of a team | Partly | `crates/gathr/src/bin/gathrd/turns/driver.rs` `Driver` | `crates/gathr/tests/codex.rs::codex_models_include_every_page_and_default_efforts` | Both run. No test puts a Codex agent in a team. The code asks "is this Codex?" in about twenty places outside the driver |
| 9.2 | Both get the same Gathr tools and the same briefing | Partly | `crates/gathr/src/bin/gathrd/turns/process.rs` `spawn`, `crates/gathr/src/bin/gathrd/tools/briefing.rs` `text` | `crates/gathr/tests/tools.rs::each_turn_records_the_briefing_its_engine_was_given_and_whats_off` | The code gives both the same. The test holds Claude Code's half. For Codex a test holds that its engine reaches a Gathr tool, not what it is briefed |
| 9.3 | Approvals behave alike | Partly | `crates/gathr/src/claude/command.rs` `mode`, `crates/gathr/src/codex/command.rs` `policy` | `crates/gathr/tests/approvals.rs::gathr_asks_before_a_push_or_a_write_outside_and_full_turns_its_hook_off` | They differ: Ask is a read-only sandbox on Codex; Codex confines shell writes and Claude Code doesn't; in Auto a write outside asks you on Claude Code and Codex's own reviewer on Codex |
| 9.4 | The model list is each engine's own answer, as exact models | Built | `crates/gathr/src/bin/gathrd/models.rs` `listed`, `ui/src/settings/models.ts` `choices` | `crates/gathr/tests/models.rs::the_list_comes_from_the_engine_once_and_again_after_sign_in` | Settings offers exact models only. The service's list still carries the family rows, for agents saved on one before |
| 9.5 | Auto-compact reaches an agent on either engine, on any model | Built | `crates/gathr/src/context.rs` `point_for` | `crates/gathr/tests/context.rs::an_agent_on_a_latest_or_the_engines_default_starts_with_its_point_once_its_window_is_seen`, `crates/gathr/tests/context.rs::codex_opens_its_thread_with_auto_compact` | |
| 9.6 | Gathr reaches GitHub for agents, so an engine with no login of its own can still work on a PR | Built | `crates/gathr/src/bin/gathrd/github.rs` `pr`, `update` | `crates/gathr/tests/github.rs::an_agent_reads_a_pr_and_why_its_ci_failed_bounded_and_with_secrets_hidden` | With your own `gh` sign-in on the computer |
| 9.7 | Mixing engines is used: a review by a different provider | Written | | | Any agent can be the reviewer. Nothing proposes or prefers one on another engine or account |
| 9.8 | Team scenarios run once for each engine | Partly | `crates/gathr/tests/steps.rs` | `crates/gathr/tests/steps.rs::a_codex_turns_steps_carry_the_command_out_of_its_shell_the_files_and_the_search` | Steps, thinking, context and recovery have a Codex case. The team, coordination and merge tests only ever start Claude Code |

## 10. Quality and speed in balance

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 10.1 | Review is never skipped: a merge through Gathr needs a clean review of what is merged | Built | `crates/gathr/src/bin/gathrd/merging.rs` `merge`, `reviewed_is_head` | `crates/gathr/tests/merge.rs::each_unmet_condition_refuses_with_a_reference_and_merges_nothing`, `crates/gathr/tests/merge/review_updates.rs::an_author_commit_after_review_still_needs_review` | The review is of the commit being merged, or of an earlier one when everything since only brings in the base. A merge typed outside Gathr isn't guarded ([0043](decisions/0043-checks-and-merge.md), C5) |
| 10.2 | Proof is never skipped: Gathr runs a project's checks at the commit and keeps the result | Built | `crates/gathr/src/bin/gathrd/check_runs.rs` `run_now` | `crates/gathr/tests/check_runs.rs::a_run_sees_the_commit_alone_never_what_is_left_uncommitted` | |
| 10.3 | The size of the process fits the size of the ask | Written | | | Nothing sizes an ask (2.4). Every ask takes the same path |
| 10.4 | Nobody waits on nothing: time with work owed and nothing running is measured | Written | | | Not measured. The nearest are stale tasks and the flag on a silent turn (6.3) |
| 10.5 | A PR behind its base is brought up to date before it merges | Built | `crates/gathr/src/bin/gathrd/github.rs` `update`, `crates/gathr/src/bin/gathrd/merging.rs` `merge` | `crates/gathr/tests/github.rs::only_the_reviewer_updates_a_pr_and_never_one_that_moved_closed_or_was_retargeted`, `crates/gathr/tests/merge.rs::each_unmet_condition_refuses_with_a_reference_and_merges_nothing` | The first test holds the update, the second that a PR behind its base isn't merged |

## 11. No rulebook needed, and Gathr measures it

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 11.1 | A new project works on defaults | Partly | `crates/gathr/src/bin/gathrd/tools/briefing.rs` `text` | `crates/gathr/tests/tools.rs::each_turn_records_the_briefing_its_engine_was_given_and_whats_off` | Every agent gets one default briefing. No `AGENTS.md` is written from questions, and no role is told how it works. This repo's own `AGENTS.md` still carries 22 rules that stand in for the product |
| 11.2 | Gathr counts how often you had to correct the room, and shows it | Written | | | Nothing counts a correction |
| 11.3 | A scorecard in the app shows numbers from Gathr's own events | Written | | | No scorecard or analytics page |
| 11.4 | Friction an agent or the person reports is recorded by Gathr | Written | | | Nothing records it. An agent ends a reply with a "Friction:" line, and the lead copies it into a file by hand (the Friction rule in [`AGENTS.md`](../AGENTS.md)). `read_chat` keeps the end of a message it cuts, so the line reaches a reader |

## 12. Open both ways

| Part | What the promise says | State | Code | Test | Missing, or against |
|---|---|---|---|---|---|
| 12.1 | An outside AI you allow can bring something to your team | Written | | | Against: Gathr's tools open only to an engine Gathr started, with that engine's own token. A test holds that every other token is refused: `an_engines_token_reaches_gathrs_tools_as_its_agent_until_the_engine_exits` |
| 12.2 | It shows in the chat as a named guest | Written | | | No guest |
| 12.3 | It hears back over the same connection | Written | | | Nothing to answer |
| 12.4 | Your agents can reach outside tools you allow | Partly | `crates/gathr/src/claude/command.rs` `DENIED_TOOLS`, `crates/gathr/src/codex/command.rs` | | An engine's own web tools and its shell work. Against: connectors, plugins and other MCP servers are switched off, with no setting to allow one ([0024](decisions/0024-only-what-gathr-gives.md)). No test holds what is on |
| 12.5 | Nothing from outside speaks as you | Written | | | Nothing comes from outside yet. Inside, agents hold tools that answer an approval or a question for you, and a test holds only that an agent can't answer its own |
