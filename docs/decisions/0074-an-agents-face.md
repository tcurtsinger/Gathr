# 0074: An agent's face

Status: accepted (owner, 2026-11-17: "Yes, use them" to the faces mock-up's sections 1 to 9, after "these Nice Avatars are definitely the best. As long as we can generate based on Gender, and Skin Tone"; then "Copy it in" over installing the library). Date: 2026-11-17.

Built: yes. Proof: `ui/src/faces/options.test.ts`, `ui/src/faces/Drawing.test.tsx`, `ui/src/settings/Agents.test.tsx`, `crates/gathr/tests/agents.rs` (`a_new_agents_face_seed_is_its_name_and_a_rename_keeps_it`, `a_faces_gender_skin_tone_and_seed_are_saved_and_bad_ones_refused`, `a_removed_agent_keeps_its_face_for_its_old_messages`), `crates/gathr/src/db.rs` (`an_agents_face_starts_from_its_name`). How a face looks is proved by pictures, not tests.

Amends [0038](0038-summary-holds-the-plan.md) and [0030](0030-ui-foundation.md) (below). Serves promise 6 of [the charter](../charter.md#the-promises): who said what, at a glance.

## The choice

- **Every agent has a drawn face, the same everywhere a face shows.** The drawings are Nice Avatar's (react-nice-avatar 1.5.0, MIT), which draws the Avatar Illustration System by Micah Lanier (CC BY 4.0). A face sits on a pale tint of the agent's colour: 30% of the colour in white, mixed in OKLab. One component, `Face` (`ui/src/components/Face.tsx`), takes an agent and a size: 36 heading its panel, 24 on a message, in Summary's rows, the approval dock and a project's team, 20 in a stack, 16 on its workspace tab, 56 in its form. Where its name is beside it the face is hidden from screen readers; where it stands alone it carries the name. The person and Gathr keep what they had.
- **The drawings are copied in, not installed.** `ui/src/faces` holds Gathr's own components written from the library's `src` at tag `v1.5.0` (commit `4cae947`): every shape as drawn, each part a nested `<svg>` at the box the library's CSS gave it, worked out once on a 100 by 100 face, so a face is one `<svg>` instead of HTML boxes. The few colours the library worked out with chroma-js (a mohawk's lighter side, `brighten(2)` of the hair; a hoody's or polo's lighter panels, `brighten(1)` of the shirt) are worked out once, by chroma-js's own Lab arithmetic, and kept beside their colours. Each face's masks take ids of their own, where the library gave every face the same ones. No package is added.
- **One generator draws a face from a seed, a gender and a skin tone** (`ui/src/faces/options.ts`, the mock-up's `nice-gen.js`). Every option takes its own hash of the seed (FNV-1a of the option's name and the seed, then a finaliser), so no two options move together. Gender (man, woman) and skin tone (six, light to deep) are inputs; when one isn't set, the seed picks it from its own hash. A man's and a woman's hair lists are lined up, so a seed never has the same hair both ways, and a woman always has lashes, so the choice shows under a hat. Hats come 2 in 8 and glasses 2 in 6. A hair or hat colour gives way to the next on the tone it melts into: white on the lightest, ginger on the two middle, brown on the deepest. No blue, cyan or green anywhere: those colours mean a state in Gathr.
- **Gender and skin tone are the person's settings.** Settings > Agents has Face (the face and Shuffle), Gender (Man, Woman) and Skin tone (six swatches), saved with the form's Save as every field there is. Shuffle picks a new seed. A gender or tone never set shows what the seed picked, and once chosen it stays.
- **The service keeps three fields and nothing drawn.** On an agent: `face_seed` (its name when made; Shuffle replaces it; a rename never does), `gender` (`man` or `woman`, empty until set) and `skin` (1 to 6, empty until set). A number, not a colour: the service needs no colour list, and the window owns the drawing. Through `POST`, `GET` and `PATCH /api/agents`, `null` clearing a gender or tone; a bad value is refused with a reference. A removed agent keeps them in `GET /api/agents`' `removed`, so its old messages keep its face. Migration 0052 adds the columns; an agent from before starts from its name.
- **The lists are fixed.** Gathr stores the seed, not the options a face was drawn with. A change to any list (its colours, its styles, their order or length), to the hash or to the skip table re-picks that feature for existing agents, and their faces change. Such a change is a decision of its own that says which faces move. A gender or tone the person set never moves.
- **The agent's colour comes from its id, with no setting.** Five colours, the mock-up's (a red-pink, a green, an amber, a violet and a blue, each deeper in light), picked by the id's hash. A room of more than five agents shares colours.
- **A face is drawn once for what it is drawn from.** A face redraws only when its seed, gender, tone or size change: a streamed token or a new message never redraws one.
- **The credit is in the window and the notice in the code.** Settings > General, under the version: "Faces: Avatar Illustration System by Micah Lanier, CC BY 4.0, changed", the work and the licence linked. The library's MIT copyright and permission notice is `ui/src/faces/LICENSE`, and each copied file says what it was adapted from.

## Why

- The owner chose the drawn faces over coloured initials and DiceBear's styles, on the mock-up's pictures, and asked that a face follow gender and skin tone.
- The library left to itself picks every option from one number, so sex, skin and ears always move together and there are six kinds of face; its own colours include the blue, cyan and green that mean a state. The generator gives the range, and gender and skin tone become the person's choice.
- Copying in: the library's last release was 2023-11; installing it brings seven packages (react-nice-avatar, chroma-js, prop-types, and prop-types' own loose-envify, object-assign, react-is 16 and js-tokens) for a class component with prop-types. Its drawing code is small, and MIT allows it.
- Seed only: one field the person can't get wrong, and a face that is the same in every window. The price, fixed lists, is written down above.
- One colour per agent, the same everywhere, names it at a glance; a tint keeps it quiet beside the marks a later change adds.

## Turned down

- **Installing react-nice-avatar:** the seven packages, and a face still picked by the library at random unless every option is passed.
- **DiceBear's styles** (Micah, Dylan, Notionists, Clay, Thumbs) and **Multiavatar:** shown in the mock-up's record, sections 10 to 19; not picked.
- **Coloured initials:** today's initials on the agent's colour; not picked.
- **Storing the options a face was drawn with,** so the lists could change: more to store and to move, for a change nobody asked for.
- **Any as a gender:** every face still reads as a man or a woman by its hair, so it would only widen the range, which Shuffle does.

## Changes to other decisions

- [0038](0038-summary-holds-the-plan.md): Summary's button stacks 20px faces, not 18px initials, and its rows and the agent's tab and panel show faces.
- [0030](0030-ui-foundation.md): the registry's SpeakerIdentity no longer draws an agent; Gathr's speaker header composes the face and the name from outside the registry folder.
