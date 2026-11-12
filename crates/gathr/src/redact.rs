//! Removes secrets from text before it's written anywhere (docs/decisions/0011-event-record.md).
//! Each removal leaves `[redacted:<kind>]`.

pub mod stream;

/// Flags whose value is a secret.
const SECRET_FLAGS: &[&str] = &["token", "key", "secret", "password"];

/// Keys whose string value is a secret whatever it looks like (`refresh_token`, `Authorization`).
const SECRET_KEYS: &[&str] = &[
    "token",
    "secret",
    "password",
    "apikey",
    "api_key",
    "authorization",
    "credential",
    "cookie",
];

/// Where a word ends: whitespace, or the punctuation JSON and shell quoting put around a value,
/// so a key inside `{"key":"sk-…"}` or a fragment of streamed tool input is still one word.
fn ends_word(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '"' | '\'' | '{' | '}' | '[' | ']' | '(' | ')' | ',' | ';' | ':' | '`' | '\\'
        )
}

/// What's already hidden stays as it is, so text redacted twice (a step's text, then the event
/// that carries it) reads the same: a mark is passed over whole, and stands for the secret a
/// `--token` or `Bearer` before it was waiting for.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut hide_next: Option<&'static str> = None;
    let mut rest = text;
    while let Some((at, end)) = mark(rest) {
        redact_part(&rest[..at], &mut hide_next, &mut out);
        hide_next = None;
        out.push_str(&rest[at..end]);
        rest = &rest[end..];
    }
    redact_part(rest, &mut hide_next, &mut out);
    out
}

/// Where the next `[redacted:<kind>]` starts and ends.
fn mark(text: &str) -> Option<(usize, usize)> {
    const OPENS: &str = "[redacted:";
    let at = text.find(OPENS)?;
    let kind = &text[at + OPENS.len()..];
    let len = kind.find(']')?;
    let named = len > 0 && kind[..len].bytes().all(|b| b.is_ascii_lowercase());
    named.then_some((at, at + OPENS.len() + len + 1))
}

fn redact_part(text: &str, hide_next: &mut Option<&'static str>, out: &mut String) {
    for piece in text.split_inclusive(ends_word) {
        let word = piece.trim_end_matches(ends_word);
        let gap = &piece[word.len()..];
        if word.is_empty() {
            out.push_str(piece);
            continue;
        }
        if let Some(kind) = hide_next.take() {
            out.push_str(&format!("[redacted:{kind}]{gap}"));
            continue;
        }
        match classify(word) {
            Some(Shape::Whole(kind)) => out.push_str(&format!("[redacted:{kind}]")),
            Some(Shape::Value(name)) => out.push_str(&format!("{name}=[redacted:env]")),
            Some(Shape::Flag(flag)) => out.push_str(&format!("{flag}[redacted:flag]")),
            Some(Shape::Next(kind)) => {
                *hide_next = Some(kind);
                out.push_str(word);
            }
            None => out.push_str(word),
        }
        out.push_str(gap);
    }
}

/// Every string in a JSON value redacted, and a secret-named key's string hidden whole. Frames
/// and event attributes go through here, not `redact` on their text: in JSON text a secret sits
/// inside quotes and escapes that `redact` alone can't see past.
pub fn redact_json(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    let secret_key = |k: &str| {
        let k = k.to_ascii_lowercase();
        SECRET_KEYS.iter().any(|s| k.contains(s))
    };
    match value {
        Value::String(s) => Value::String(redact(s)),
        Value::Array(items) => Value::Array(items.iter().map(redact_json).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| match v {
                    Value::String(s) if secret_key(k) && !s.is_empty() => {
                        (k.clone(), Value::from("[redacted:named]"))
                    }
                    _ => (k.clone(), redact_json(v)),
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

enum Shape<'a> {
    /// The word itself is a secret.
    Whole(&'static str),
    /// `NAME=value`: keep the name, hide the value.
    Value(&'a str),
    /// `--flag=value`: keep the flag and its `=`, hide the value.
    Flag(&'a str),
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
    // The flag's name decides, with its value after a space or an `=`: `--port=47101` stays.
    if let Some(flag) = word.strip_prefix("--") {
        let (name, value) = flag.split_once('=').unwrap_or((flag, ""));
        let lower = name.to_ascii_lowercase();
        if SECRET_FLAGS.iter().any(|s| lower.contains(s)) {
            return Some(match value {
                // `--token abc`, and `--token="abc"`, whose quote ends the word.
                "" => Shape::Next("flag"),
                _ => Shape::Flag(&word[..word.len() - value.len()]),
            });
        }
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
    use super::{redact, redact_json};
    use serde_json::{Value, json};

    /// Real frame shapes (Claude Code and Codex) with a secret where an agent's work can put one.
    #[test]
    fn secrets_inside_frame_values_are_redacted() {
        let frames = [
            json!({ "type": "user", "message": { "role": "user", "content": "sk-ant-api03-abcdefghijklmnopqrst" } }),
            json!({ "type": "assistant", "message": { "content": [{ "type": "tool_use", "name": "Bash",
                "input": { "command": "curl -H \"Authorization: Bearer abc123secret\" x" } }] } }),
            json!({ "type": "stream_event", "event": { "type": "content_block_delta",
                "delta": { "type": "input_json_delta", "partial_json": "{\"key\":\"sk-proj-0123456789abcdefgh\"}" } } }),
            json!({ "method": "item/started", "params": { "item": { "type": "mcpToolCall",
                "arguments": { "headers": { "Authorization": "Basic dXNlcjpwYXNz" }, "refresh_token": "rt-1" } } } }),
        ];
        for frame in frames {
            let line = frame.to_string();
            let body = redact_json(&frame).to_string();
            for secret in [
                "sk-ant-api03",
                "abc123secret",
                "sk-proj-0123",
                "dXNlcjpwYXNz",
                "rt-1",
            ] {
                assert!(
                    !body.contains(secret),
                    "{secret} survived in {body} (from {line})"
                );
            }
            assert!(body.contains("[redacted:"), "nothing was marked in {body}");
            serde_json::from_str::<Value>(&body).expect("the redacted frame is still JSON");
        }
    }

    #[test]
    fn frame_values_without_secrets_are_untouched() {
        let frame = json!({ "type": "result", "usage": { "input_tokens": 12, "output_tokens": 3 },
            "result": "Done: wrote note.txt", "session_id": "8947397a-6c50-409f-9253-43c09abad99c" });
        assert_eq!(redact_json(&frame), frame);
    }

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

    /// A secret flag given its value with `=`: the name decides, as it does for `--flag value`.
    #[test]
    fn a_secret_flags_value_after_an_equals_sign_is_redacted() {
        let cases = [
            (
                "gh --token=abc123 pr list",
                "gh --token=[redacted:flag] pr list",
            ),
            ("x --api-key=12345", "x --api-key=[redacted:flag]"),
            ("x --PASSWORD=hunter2 y", "x --PASSWORD=[redacted:flag] y"),
            ("x --token=\"abc123\" y", "x --token=\"[redacted:flag]\" y"),
        ];
        for (input, expected) in cases {
            assert_eq!(redact(input), expected, "input: {input:?}");
        }
        let plain = "cargo run --port=47101 --data=test-data --color=always --jobs=4 -k=v";
        assert_eq!(redact(plain), plain);
    }

    /// A step's text is redacted when it's kept, then again on the event that carries it: the
    /// second pass took "bearer" inside `[redacted:bearer]` for the word before a secret.
    #[test]
    fn text_already_redacted_stays_as_it_is() {
        let text = "curl --token=abc123 -H \"Authorization: Bearer abc.def\" https://example.test \
                    KEY_ID=x --password hunter2 sk-proj-0123456789abcdefgh done";
        let once = redact(text);
        assert_eq!(redact(&once), once);
        // A mark that came in someone's text ends a wait for a secret too.
        assert_eq!(
            redact("--token [redacted:flag] next"),
            "--token [redacted:flag] next"
        );
    }

    #[test]
    fn ordinary_text_is_untouched() {
        let text =
            "start failed: file is not a database (path X:\\data\\gathr.db) a=b --port 47101\n";
        assert_eq!(redact(text), text);
    }
}
