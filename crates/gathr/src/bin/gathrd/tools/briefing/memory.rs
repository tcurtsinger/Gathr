//! The briefing's part on Memory: the notes the agent has as its engine starts, and when to keep one.
//! A note kept later reaches it at its next engine's start.

use gathr::memory::{Known, Place};

/// Each kind's notes shown at most, newest first, and their characters at most.
const MOST: usize = 60;
const CHARS: usize = 8_000;

pub(super) const ASK: &str = "When you learn something that will still matter in later chats (how the person likes \
to work, a lasting fact about this project, a lesson about doing your work, how to use a tool the person added), \
call note with one short line. Not what the code, the chat or a file already says, and not what only matters today.";

/// A part for each place that has notes: its heading, its notes, and what was left out.
pub(super) fn known(known: &[Known], person: Option<&str>) -> Vec<String> {
    known
        .iter()
        .map(|(place, name, path, notes)| {
            let heading = match place {
                Place::You => format!("What you know about {}:", person.unwrap_or("the person")),
                Place::Project(_) => "About this project:".to_string(),
                Place::Agent(_) => "Your own notes from earlier work:".to_string(),
                Place::Tool(_) => format!("About the tool {name}:"),
            };
            let mut lines = vec![heading];
            let mut used = 0;
            for note in notes.iter().take(MOST) {
                used += note.text.chars().count() + 3;
                if used > CHARS {
                    break;
                }
                lines.push(format!("- {}", note.text));
            }
            let left = notes.len() + 1 - lines.len();
            if left > 0 {
                lines.push(format!(
                    "{left} more in \"{}\": read it for them.",
                    path.display()
                ));
            }
            lines.join("\n")
        })
        .collect()
}
