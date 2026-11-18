//! What the team at work says: the person's asks, the lead's answers and parts, and the files and
//! commands its agents work with. All of it made up.

/// An ask: what the person says, the lead's answer, its parts (what, why), and a question for the
/// person with its choices, the pick first.
pub(super) struct Ask {
    pub(super) said: &'static str,
    pub(super) answer: &'static str,
    pub(super) plan: bool,
    pub(super) parts: &'static [(&'static str, &'static str)],
    pub(super) question: Option<(&'static str, &'static [&'static str])>,
}

pub(super) const ASKS: [Ask; 10] = [
    Ask {
        said: "Add tags to notes, so I can find my trip notes again",
        answer: "Tags it is. This touches the database and the list, so I'll write a plan first and Ivo checks it.",
        plan: true,
        parts: &[
            ("Add the tags table", "Notes need tags stored with them"),
            (
                "Show tags on each note",
                "You read a note's tags where you read the note",
            ),
            (
                "Filter the list by tag",
                "Finding trip notes is the point of tags",
            ),
        ],
        question: Some((
            "Should a tag be one word, or may it have spaces?",
            &["One word", "Spaces allowed"],
        )),
    },
    Ask {
        said: "The export button does nothing on Windows",
        answer: "That's a repair. Rowan looks for why, then we fix the path and cover it with a test.",
        plan: false,
        parts: &[
            (
                "Find why export fails on Windows",
                "Export works on the Mac, so it's the platform",
            ),
            (
                "Fix the save dialog's path",
                "The dialog hands back a path with the wrong slashes",
            ),
        ],
        question: None,
    },
    Ask {
        said: "Sync gets stuck at 99% after the laptop sleeps",
        answer: "Sol has seen this one before. Three steps: reproduce it, resume after a wake, and show honest progress.",
        plan: false,
        parts: &[
            (
                "Reproduce the stuck sync after sleep",
                "We can't fix what we can't make happen",
            ),
            (
                "Resume uploads after a wake",
                "The upload queue forgets its place when the laptop sleeps",
            ),
            (
                "Show sync progress honestly",
                "99% for an hour reads as broken",
            ),
        ],
        question: None,
    },
    Ask {
        said: "Make search find words with accents, cafe should find café",
        answer: "Search folds case already; accents are next. Tamsin takes the index, Sol the tests.",
        plan: false,
        parts: &[
            (
                "Fold accents in the search index",
                "cafe and café should be one word to search",
            ),
            (
                "Rebuild the index for old notes",
                "Notes saved before the change need folding too",
            ),
            ("Test search with accented words", "So it stays fixed"),
        ],
        question: None,
    },
    Ask {
        said: "Dark mode: the sidebar is too bright at night",
        answer: "The sidebar uses the light token. Tamsin swaps it and checks both themes.",
        plan: false,
        parts: &[
            (
                "Use the dark sidebar colour",
                "The sidebar ignores the theme",
            ),
            (
                "Check every screen in dark mode",
                "Where one token was wrong, others may be",
            ),
            (
                "Add a screenshot test for the sidebar",
                "So the colour can't slip back",
            ),
        ],
        question: None,
    },
    Ask {
        said: "Import notes from a folder of Markdown files",
        answer: "Import is new work with a few choices in it. A plan first, and a question for you about titles.",
        plan: true,
        parts: &[
            (
                "Read a folder of Markdown files",
                "Each file becomes a note",
            ),
            (
                "Keep each file's dates",
                "Old notes should keep their place in time",
            ),
            (
                "Show what was imported",
                "You should see what came in and what didn't",
            ),
        ],
        question: Some((
            "Where should an imported note's title come from?",
            &["The first heading", "The file name"],
        )),
    },
    Ask {
        said: "The app takes 6 seconds to open with 3,000 notes",
        answer: "Too slow. Rowan measures first, then we fix what the numbers show.",
        plan: false,
        parts: &[
            (
                "Measure opening with 3,000 notes",
                "Know where the 6 seconds go",
            ),
            (
                "Load the list in pages",
                "The list reads every note at start",
            ),
            (
                "Keep the open time under a second",
                "A test holds the number",
            ),
        ],
        question: Some((
            "What open time should we hold to?",
            &["Under a second", "Under two seconds"],
        )),
    },
    Ask {
        said: "Add a pinned section at the top of the list",
        answer: "Small and clear: a pin on each note, and pinned notes first.",
        plan: false,
        parts: &[
            ("Add a pin to each note", "You pick what stays on top"),
            ("Show pinned notes first", "Pinned means first in the list"),
            (
                "Keep pins after a sync",
                "A pin set on one laptop shows on the other",
            ),
        ],
        question: None,
    },
    Ask {
        said: "Spell check underlines words inside code blocks",
        answer: "Code isn't prose. Sol turns spell check off inside code and checks the edge cases.",
        plan: false,
        parts: &[
            (
                "Skip spell check in code blocks",
                "Code words aren't misspelt",
            ),
            ("Skip inline code too", "Same rule for `inline` code"),
            (
                "Test a note mixing code and prose",
                "The prose around code still gets checked",
            ),
        ],
        question: Some((
            "Spell check inside code comments too?",
            &["No, skip all code", "Yes, check comments"],
        )),
    },
    Ask {
        said: "Write the release notes for 0.4",
        answer: "Tamsin drafts from the merged work, Rowan checks the wording.",
        plan: false,
        parts: &[
            (
                "Draft the 0.4 release notes",
                "Everything merged since 0.3, in plain words",
            ),
            (
                "Check the release notes' wording",
                "Short and plain, no internal names",
            ),
        ],
        question: None,
    },
];

pub(super) const FILES: &[&str] = &[
    "src/notes/tags.rs",
    "src/notes/list.rs",
    "src/sync/queue.rs",
    "src/search/fold.rs",
    "src/export/save.rs",
    "ui/src/sidebar/Sidebar.tsx",
    "ui/src/list/NoteList.tsx",
    "tests/search.rs",
    "tests/sync_sleep.rs",
    "docs/release-0.4.md",
];
pub(super) const COMMANDS: &[&str] = &[
    "git status --short",
    "cargo test --test search",
    "cargo clippy --all-targets -- -D warnings",
    "npx vitest run src/list",
    "rg -n 'fn export' src",
];
pub(super) const SAID: &[&str] = &[
    "Done, and the tests pass.",
    "That works now; I added a test for it.",
    "Found it: the path came back with the wrong slashes.",
    "Measured three times; the numbers are in the PR.",
    "It's in. One thing to watch: old notes need the rebuild once.",
    "Checked both themes at both sizes.",
];
pub(super) const NAMES: [&str; 5] = ["Mira", "Rowan", "Sol", "Tamsin", "Ivo"];
