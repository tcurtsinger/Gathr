//! Removes secrets from text before it's written anywhere (docs/decisions/0011-event-record.md).
//! Each removal leaves `[redacted:<kind>]`.

/// Flags whose next word is a secret.
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

pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut hide_next: Option<&str> = None;
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

    #[test]
    fn ordinary_text_is_untouched() {
        let text =
            "start failed: file is not a database (path X:\\data\\gathr.db) a=b --port 47101\n";
        assert_eq!(redact(text), text);
    }
}
