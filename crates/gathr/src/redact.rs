//! Removes secrets from text before it's written anywhere (docs/decisions/0011-event-record.md).
//! Each removal leaves `[redacted:<kind>]`.

/// Flags whose next word is a secret.
const SECRET_FLAGS: &[&str] = &["token", "key", "secret", "password"];

pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut hide_next: Option<&str> = None;
    for piece in text.split_inclusive(char::is_whitespace) {
        let word = piece.trim_end();
        let gap = &piece[word.len()..];
        if word.is_empty() {
            out.push_str(piece);
            continue;
        }
        if let Some(kind) = hide_next.take() {
            out.push_str(&format!("[redacted:{kind}]{gap}"));
            continue;
        }
        let bare =
            word.trim_matches(|c: char| matches!(c, '"' | '\'' | '(' | ')' | ',' | ';' | '`'));
        match classify(bare) {
            Some(Shape::Whole(kind)) => {
                out.push_str(&word.replace(bare, &format!("[redacted:{kind}]")))
            }
            Some(Shape::Value(name)) => {
                out.push_str(&word.replace(bare, &format!("{name}=[redacted:env]")))
            }
            Some(Shape::Next(kind)) => {
                hide_next = Some(kind);
                out.push_str(word);
            }
            None => out.push_str(word),
        }
        out.push_str(gap);
    }
    out
}

enum Shape<'a> {
    /// The word itself is a secret.
    Whole(&'static str),
    /// `NAME=value`: keep the name, hide the value.
    Value(&'a str),
    /// The next word is a secret.
    Next(&'static str),
}

fn classify(word: &str) -> Option<Shape<'_>> {
    if word.starts_with("sk-") && word.len() >= 20 {
        return Some(Shape::Whole("key"));
    }
    if is_jwt(word) {
        return Some(Shape::Whole("jwt"));
    }
    if word.eq_ignore_ascii_case("bearer") {
        return Some(Shape::Next("bearer"));
    }
    if let Some(flag) = word.strip_prefix("--")
        && !flag.contains('=')
        && SECRET_FLAGS
            .iter()
            .any(|s| flag.to_ascii_lowercase().contains(s))
    {
        return Some(Shape::Next("flag"));
    }
    let (name, value) = word.split_once('=')?;
    let env_name = name.len() >= 2
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    (env_name && !value.is_empty()).then_some(Shape::Value(name))
}

fn is_jwt(word: &str) -> bool {
    let parts: Vec<&str> = word.split('.').collect();
    parts.len() == 3
        && parts[0].starts_with("eyJ")
        && parts.iter().all(|p| {
            p.len() >= 4
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn each_secret_shape_is_redacted() {
        let cases = [
            (
                "key sk-ant-api03-abcdefghijklmnop end",
                "key [redacted:key] end",
            ),
            (
                "Authorization: Bearer abc.def.ghi",
                "Authorization: Bearer [redacted:bearer]",
            ),
            (
                "got eyJhbGciOi.eyJzdWIiOiIx.c2lnbmF0dXJl\n",
                "got [redacted:jwt]\n",
            ),
            (
                "run ANTHROPIC_API_KEY=xyz claude",
                "run ANTHROPIC_API_KEY=[redacted:env] claude",
            ),
            (
                "claude --api-key 12345 -p",
                "claude --api-key [redacted:flag] -p",
            ),
            (
                "value \"sk-proj-0123456789abcdef\",",
                "value \"[redacted:key]\",",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(redact(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn ordinary_text_is_untouched() {
        let text =
            "start failed: file is not a database (path X:\\data\\gathr.db) a=b --port 47101\n";
        assert_eq!(redact(text), text);
    }
}
