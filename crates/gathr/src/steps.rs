//! What a step in a turn's work ran, as the chat keeps it: a command's first line, a file's path,
//! a search's words (docs/decisions/0041-what-a-step-ran.md). Each driver picks the field; the
//! cut and the cleaning are here, once.

use serde::Serialize;

use crate::redact::redact;
use crate::text;

const TEXT: usize = 200;
const TITLE: usize = 80;

/// What a tool call ran, and the engine's own words for it when it gives some.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepInput {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// `text` and `title` as a step keeps them; nothing when the text is empty.
pub fn input(text: &str, title: Option<&str>) -> Option<StepInput> {
    Some(StepInput {
        text: kept(text, TEXT)?,
        title: title.and_then(|title| kept(title, TITLE)),
    })
}

/// The first line, secrets hidden, at most `limit` characters; "…" where more followed. Hidden
/// before the cut, so a key the cut would split is still seen whole.
fn kept(text: &str, limit: usize) -> Option<String> {
    let line = text::first_line(text)?;
    let more = text.trim().len() > line.len();
    Some(text::cut(&redact(line), limit, more))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(of: &str) -> String {
        input(of, None).unwrap().text
    }

    #[test]
    fn a_short_one_line_command_is_kept_as_written() {
        assert_eq!(text("git status --short"), "git status --short");
        assert_eq!(input("", None), None);
        assert_eq!(input(" \n ", Some("Title")), None);
    }

    #[test]
    fn only_the_first_line_is_kept_and_the_cut_shows() {
        assert_eq!(text("cargo fmt --all\ncargo clippy\n"), "cargo fmt --all…");
        assert_eq!(text("\n  git fetch  \n"), "git fetch");
    }

    #[test]
    fn a_long_line_is_cut_to_200_characters_on_a_character_boundary() {
        // 2 bytes each: a cut by bytes would split one.
        let long = "é".repeat(300);
        let cut = text(&long);
        assert_eq!(cut.chars().count(), 200);
        assert_eq!(cut, format!("{}…", "é".repeat(199)));
        assert_eq!(text(&"é".repeat(200)), "é".repeat(200));
    }

    #[test]
    fn a_title_is_cut_to_80_characters() {
        let step = input("ls", Some(&"word ".repeat(40))).unwrap();
        let title = step.title.unwrap();
        assert_eq!(title.chars().count(), 80, "{title}");
        assert!(title.ends_with('…'), "{title}");
        assert_eq!(input("ls", Some("  ")).unwrap().title, None);
    }

    #[test]
    fn each_secret_shape_is_hidden() {
        let cases = [
            (
                "curl -H 'Authorization: Bearer abc123secret' https://example.test",
                "curl -H 'Authorization: Bearer [redacted:bearer]' https://example.test",
            ),
            (
                "deploy --key sk-test-0123456789abcdefgh",
                "deploy --key [redacted:flag]",
            ),
            ("echo sk-test-0123456789abcdefgh", "echo [redacted:key]"),
            (
                "API_TOKEN=made-up-value npm test",
                "API_TOKEN=[redacted:env] npm test",
            ),
            (
                "gh auth login --with-token=made-up-value",
                "gh auth login --with-token=[redacted:flag]",
            ),
            (
                "echo eyJhbGciOi.eyJzdWIiOiIx.c2lnbmF0dXJl",
                "echo [redacted:jwt]",
            ),
        ];
        for (command, shown) in cases {
            assert_eq!(text(command), shown, "command: {command}");
        }
    }

    /// The cut comes after the hiding: cut first, the 20 characters a key needs to be known as
    /// one could fall past the limit and leave its start showing.
    #[test]
    fn a_key_the_cut_would_split_is_hidden_whole() {
        let command = format!("{} sk-test-0123456789abcdefgh", "x".repeat(190));
        assert_eq!(text(&command), format!("{}…", "x".repeat(190)));
    }

    /// A mark the cut lands in goes whole: half of one would read as text.
    #[test]
    fn a_secret_at_the_cut_leaves_no_half_mark() {
        let shown = text(&format!("{} --token=abc", "x".repeat(185)));
        assert_eq!(shown, format!("{} --token=…", "x".repeat(185)));
    }

    #[test]
    fn the_body_holds_text_and_a_title_only_when_there_is_one() {
        let with = input("ls", Some("List files")).unwrap();
        let json = serde_json::to_string(&with).unwrap();
        assert_eq!(json, r#"{"text":"ls","title":"List files"}"#);
        let without = serde_json::to_string(&input("ls", None).unwrap()).unwrap();
        assert_eq!(without, r#"{"text":"ls"}"#);
    }
}
