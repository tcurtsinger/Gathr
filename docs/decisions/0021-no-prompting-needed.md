# 0021. Gathr works without a good prompt

Status: accepted (the owner's requirement). Date: 2026-11-07.

## Choice

Anything a person would otherwise have to put in a careful prompt to make Gathr work well is built in: in the Supervisor's briefing, in the roles, in the tools, in the service's state and in defaults. A person can type "build an export for my notes" into a chat and it runs the way a well-prompted one would.

### What's built in

The starter prompt the owner had to write to get Gathr's own first room working is the list of what Gathr must do on its own:

| The prompt had to say | Gathr does it by |
|---|---|
| Read the rulebook first | A project chat's agents work with the project's `AGENTS.md`: both engines load it from the project folder themselves (Claude Code from v2.1.277), and the briefing points to it ([0020](0020-context-per-turn.md)). By default Claude Code skips `AGENTS.md` when the project has a `CLAUDE.md` (or `CLAUDE.local.md`) that doesn't import it with `@AGENTS.md`. So Gathr sets Claude Code's project-instructions setting to `claude-md-and-agents-md` in each account's config folder ([0002](0002-login-folders.md)): both files are read, `CLAUDE.md` first, in every project, whatever it already has ([memory](https://code.claude.com/docs/en/memory#share-one-file-with-other-coding-tools)). Gathr writes no `CLAUDE.md` of its own. A new project gets its `AGENTS.md` written from a few questions (what it is, the stack if known, public or private), starting from Gathr's template of working rules; the person edits it. |
| Plan first, nothing built until approved | Plan mode is a setting on the chat and a button, and the Supervisor's briefing says to plan before building ([0010](0010-coordination-as-state.md)). |
| Break the work into parts, in parallel, each with an owner | The Supervisor's briefing: turn an ask into ordered parts, each with an owner by role, a done and a check, parallel where they can be. |
| Which member does which part | Each role (Supervisor, Reviewer, designer, engineer and so on) comes with a default "how it works" the person can edit. The Supervisor assigns by role and seniority. |
| Decisions are the person's, as tasks, with a pick | Built into tasks: only the Supervisor gives the person tasks, as a decision with choices and its pick first ([0010](0010-coordination-as-state.md)). |
| Where the lessons are | A lesson the person keeps becomes a project note, and project notes are standing rules ([0020](0020-context-per-turn.md)). Anything longer, such as a lessons file in the project, is fetched with a tool when a plan touches it, never put in every turn. |

And for what the person doesn't say at all:
- **A vague ask** gets one question from the Supervisor, with options and its pick, only when the answer changes the work. Otherwise it picks a sensible default, says so, and goes ahead.
- **No assumptions, without being told.** Every role's default way of working says to check a fact in the code, current docs or by running it before stating or building on it, and to mark what couldn't be checked as not verified. The person never has to add "look it up" to a prompt.
- **Tools explain themselves.** Each tool's description says when to use it and when not to. Misuse is refused with what to do instead ("Only the Supervisor gives the person tasks: ask your Supervisor").
- **The service nudges the agent, not the person,** from state: a turn that ended with work still open and no wait set, a PR with no proof, a plan with no checks.

### How it's measured

The analytics page shows how often each tool is used, by agent, and lists tools nobody used in the last 30 days; an unused tool is fixed (its description, its place in the briefing) or removed. It also counts turns where the person had to restate an ask or correct an agent, the signal that something a prompt would have said is still missing.

## Why

- The owner's requirement: most people, the owner included, aren't good at prompting, and Gathr must work properly anyway.
- The previous app needed a long starter prompt, written with another AI's help, before this room worked the way it should.
- In the previous app, 36 of 54 agent commands were never run in the six days counted; tools that need to be asked for by name go unused.

## Checked, and not yet

- **Checked:** Claude Code started the way SerenityOS starts it (`claude -p` with stream-json and its own `CLAUDE_CONFIG_DIR`, v2.1.318) loaded this repo's `AGENTS.md` with no `CLAUDE.md` in the folder: the first session of the room that wrote this shows it as the project instructions.
- **Not verified:** that Claude Code reads the `claude-md-and-agents-md` setting from `settings.json` in a `CLAUDE_CONFIG_DIR` folder. The docs name only `~/.claude/settings.json`, a `--settings` file and managed settings. The first slice proves it, or passes the setting with `--settings` instead.

## Turned down

- **Prompt templates the person fills in.** Still prompting, with extra steps.
- **Rewriting the person's message into a "better prompt".** It hides what the person actually said; the Supervisor asks instead, when it matters.
- **Long default instructions for every agent.** It trades bad prompting for bloated context ([0020](0020-context-per-turn.md)). What's built in lives in tools and state wherever it can, and the briefing stays short.
