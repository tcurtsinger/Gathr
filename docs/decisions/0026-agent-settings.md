# 0026: Agent settings

Status: accepted design. Implementation planning remains separate.

Agents are created and edited in Settings > Agents, inside the centred Settings
panel. Edit agent in a chat's agent picker opens that same panel at that agent.
The composer selects the recipient; engine, account, model and effort belong to
the reusable agent.

Save changes that agent in every chat where it is used. There is no per-save
Scope control. New turns use the saved assignment; a running turn keeps the one
it started with. Roles remain specific to a chat.

One editor gives reusable agents a consistent home and contextual access without
duplicating forms. One shared assignment avoids conflicting versions of an agent.

Turned down: editing inside Team or a separate chat menu editor, and changes
limited to the current chat. Those obscure whether a reusable agent has changed.

See DESIGN.md and designs/agents/v1.html. The other two previews are not chosen.
Engine discovery, durable storage and cross-chat propagation are not connected
in these previews.

## Named accounts revision

Selected model: Settings opens from a gear and Settings row at bottom left.
Gathr owns engine programs. Engines lists named accounts, each bound to one
engine and signed in independently. Add chooses engine, name, then sign-in.
Version and updates appear once per engine, separate from login rows. No custom
program path is exposed in Settings; dev/test overrides remain outside this UI.
This follows 0002's separate login folders and 0025's managed copies.

The agent editor has Account, Model and Effort. Account implies engine.
Changing account resets model and effort; saved assignments apply everywhere,
on future turns. A running turn keeps its original assignment.

Turned down: a custom engine source, separate engine and account pickers, and
one version/update control per account. These duplicate shared program state
or permit contradictory assignments.

The working layout in designs/settings-entry/v5.html awaits approval before
implementation. Add, rename, sign in/out and capability lists are simulated;
no account, engine installation or login is created by this prototype.
