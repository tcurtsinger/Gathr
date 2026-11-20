//! One line out of longer text, for a place that shows one: a chat's title, what a step ran. And a
//! long message kept to its start and end, for an agent to read.

use crate::redact::MARK;

/// The first line with text, without the space around it.
pub fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

/// `line` in at most `limit` characters, ending "…" when it was cut, or when `more` followed it.
/// Never cut inside a `[redacted:…]` mark: the cut backs up to before it.
pub fn cut(line: &str, limit: usize, more: bool) -> String {
    if !more && line.chars().count() <= limit {
        return line.to_string();
    }
    let keep = limit.saturating_sub(1);
    let mut end = line
        .char_indices()
        .nth(keep)
        .map_or(line.len(), |(at, _)| at);
    if let Some(mark) = line[..end].rfind('[')
        && line[mark..].starts_with(MARK)
        && !line[mark..end].contains(']')
    {
        end = mark;
    }
    format!("{}…", line[..end].trim_end())
}

/// Names in a sentence: "Ann", "Ann and Bo", "Ann, Bo and Cy".
pub fn list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => one.to_string(),
        [most @ .., last] => format!("{} and {last}", most.join(", ")),
    }
}

/// A message longer than `KEEP_START` and `KEEP_END` together is cut in the middle, with its id to
/// read the whole of it (0020: a tool returns what the question needs, with an id for the rest).
/// Its end stays: agents end a message with what matters to a scan, such as "Friction: …" (G176).
const KEEP_START: usize = 1_500;
const KEEP_END: usize = 400;

/// A message as `read_chat` lists it, and a turn's opening carries it (0081): whole, or its start and
/// end with what was cut between.
pub fn kept(body: &str, id: &str) -> String {
    let chars: Vec<char> = body.chars().collect();
    if chars.len() <= KEEP_START + KEEP_END {
        return body.to_string();
    }
    let start: String = chars[..KEEP_START].iter().collect();
    let end: String = chars[chars.len() - KEEP_END..].iter().collect();
    let cut = chars.len() - KEEP_START - KEEP_END;
    format!(
        "{start} [… {cut} of {} characters cut: read message {id} for all of it …] {end}",
        chars.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_line_with_text_is_taken() {
        assert_eq!(first_line("\n  \n  git fetch  \nmore"), Some("git fetch"));
        assert_eq!(first_line(" \n\t\n"), None);
    }

    #[test]
    fn a_long_line_is_cut_on_a_character_boundary() {
        // 2 bytes each: a cut by bytes would split one.
        assert_eq!(
            cut(&"é".repeat(300), 200, false),
            format!("{}…", "é".repeat(199))
        );
        assert_eq!(cut(&"é".repeat(200), 200, false), "é".repeat(200));
        assert_eq!(cut("word  and more", 6, false), "word…");
        assert_eq!(cut("short", 60, true), "short…");
    }

    /// Half a mark reads as text, and a second redaction would take it apart.
    #[test]
    fn a_cut_never_lands_inside_a_mark() {
        let line = format!("{} --token=[redacted:flag]", "x".repeat(20));
        for limit in 31..line.len() {
            let shown = cut(&line, limit, false);
            assert_eq!(shown, format!("{} --token=…", "x".repeat(20)), "at {limit}");
        }
        assert_eq!(cut(&line, line.len(), false), line);
        // A whole mark before the cut stays.
        let two = "a [redacted:key] bcdefghij";
        assert_eq!(cut(two, 20, false), "a [redacted:key] bc…");
    }
}
