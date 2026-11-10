# Visual reference source record

The owner's selected assistant-ui React prototype supplies the look. This source
snapshot records the handoff used for the design update, not a public distribution
of its local fixtures. No fixture conversation or account data was copied here.

## Authority and limits

- DESIGN.md and tokens.css adapt the prototype's appearance to Gathr.
- Accepted decisions govern service state, routing, safety and data ownership.
- Settings retains 0023's centred panel and category rail.
- Team-message arrangement awaits the owner's choice. Recipient routing is
  decided: the Supervisor by name first, other members after it, no Everyone.
  Summary replacing 0015's Team tab awaits an explicit integration decision.
- No JavaScript packages are added by this design PR. The integration proposal
  owns dependency approval, version pinning, runtime adapters and file splitting.

## Component mapping

| Gathr reference | Prototype source and production direction |
| --- | --- |
| Thread | GathrThread.tsx composes actual ThreadPrimitive and MessagePrimitive |
| Composer | ChatComposer.tsx composes the actual modular Composer |
| Reply identity/actions | SpeakerIdentity and MessageActions; arrangement pending |
| Markdown | StreamdownTextPrimitive, subject to the integration decision |
| Tool runs | ToolTimeline and ToolFallback, bound to the original turn |
| Tasks | Inline OptionList plus ElicitationForm, one shared pager |
| Summary | Inline SubagentList plus TodoList; placement conflict noted above |
| Files/documents | FileTree, Streamdown, SyntaxHighlighter and shared ResizeHandle |
| Changes/Terminal/Web | CodeDiff, TerminalBlock and WebPreview with real Gathr adapters |
| Settings | Gathr's existing centred layout, Help, Switch and common agent editor |

The 64-example gallery is a reference library, not the production feature list.
The manifest records six adaptations: FileTree, SpeakerIdentity, Button,
ElicitationForm, OptionList and SubagentList. Other registry modules keep their
upstream implementation. Import actual components when approved; do not promote
the portable HTML's static anatomy into new hand-built React components.

Official sources recorded by the prototype include
[assistant-ui's registry](https://r.assistant-ui.com/thread.json) and
[shadcn's Button registry](https://ui.shadcn.com/r/styles/new-york-v4/button.json).
The associated MIT notices are retained in notices/. The owner's typography update
replaces the prototype's Public Sans with Hanken Grotesk. Its SIL Open Font License
1.1 is in fonts/LICENSE.txt beside the unmodified Latin variable font; fonts/README.md
records the package version and integrity.

## Source SHA-256

| Source in supplied prototype | SHA-256 |
| --- | --- |
| `src/theme.css` | `3a1729c009ac8632ab2f76cecf344d0163e47e402e5d32390feb6732f89bace6` |
| `src/gathr/layout.css` | `d02b9bd8a74428554fa6842e2601bb3312842535ceb75fcbba08c3fb27ceb780` |
| `src/gathr/workspace.css` | `81e7ae1058bd639dc3c8df5992d15222839c06e5eeb594ab1d9264cb27dd698e` |
| `src/gathr/GathrThread.tsx` | `c18501b8232c9680b030911a0717fdd5cdb7f24e68e213369e6884bb34b70415` |
| `src/gathr/ChatComposer.tsx` | `af98b44ae1b023c07bcda59b42a2525d663d504e9deabf73d7b8bcf62a11464f` |
| `src/gathr/TasksContent.tsx` | `27edb6f918d93658f6bef58c8f39451ea8badf909050ff33bf7fc115df3eb2f0` |
| `src/gathr/AgentSummary.tsx` | `d723220b181dba62a2e19d08ca65473c8da60faf4cee668f2cb3fec261b2e327` |
| `upstream-manifest.json` | `5d63cad426c3c0e66761a46e48a91f421da14f6a8dd7ffcaf2dbd282e271c2a5` |
| `package-lock.json` | `15c1df9799bf129ccda75f6adcf17987e18812ec4f65ccab3eb64e02634fddc5` |

## Portable reference differences

system.html demonstrates anatomy and local controls without a React dependency.
It intentionally leaves agent-message layout pending and preserves Settings
geometry. Existing ui/ consumers keep their readable muted/accent text aliases;
the prototype's surfaces become muted-surface/accent-surface. Destructive retains
the supplied semantic value; the legacy danger text mixes it toward foreground
for small-text contrast. Focus remains visibly outlined. The preview strip,
fixture timers, sign-in data and simulated backend callbacks do not become policy.

The retired selected shell pages are chat/v14, single-chat/v12, new-chat/v12 and
start-chat/v13. They remain historical fixtures so older links and evidence open.
