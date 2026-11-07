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
