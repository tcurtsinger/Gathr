//! Streamed pieces of one value, redacted together (0011, audit 2.1.1 at review). Claude Code
//! streams a tool's input and its text in fragments, and Codex its replies and command output, so a
//! key can be split across two frames where neither looks like one. Once a fragment comes, every
//! line after it (stderr and unrelated frames too, so their order holds) is held back until a block
//! or item ends, then the run is redacted joined before any of it is written.

use serde_json::Value;

/// A frame that ends what was streaming: Claude Code's block or message end, its whole message or
/// result; Codex's item or turn end.
pub fn ends_block(frame: &Value) -> bool {
    let event = frame["event"]["type"].as_str().unwrap_or("");
    let kind = frame["type"].as_str().unwrap_or("");
    let method = frame["method"].as_str().unwrap_or("");
    matches!(event, "content_block_stop" | "message_stop")
        || matches!(kind, "assistant" | "result")
        || matches!(method, "item/completed" | "turn/completed")
}

/// The piece a frame carries, and which value it's a piece of, when it's a stream fragment.
pub fn fragment(frame: &Value) -> Option<(String, &str)> {
    if frame["type"] == "stream_event" && frame["event"]["type"] == "content_block_delta" {
        let delta = &frame["event"]["delta"];
        let text = ["partial_json", "text", "thinking"]
            .iter()
            .find_map(|key| delta[*key].as_str())?;
        return Some((format!("block {}", frame["event"]["index"]), text));
    }
    let method = frame["method"].as_str()?;
    if method.ends_with("/delta") || method.ends_with("Delta") {
        let params = &frame["params"];
        let text = params["delta"].as_str()?;
        return Some((format!("item {}", params["itemId"]), text));
    }
    None
}

fn set(frame: &mut Value, text: &str) {
    let delta = &mut frame["event"]["delta"];
    for key in ["partial_json", "text", "thinking"] {
        if delta.get(key).is_some() {
            delta[key] = text.into();
            return;
        }
    }
    frame["params"]["delta"] = text.into();
}

/// Redacts each value's fragments joined. When that changes them, the first fragment carries the
/// redacted whole and the rest are left empty, so the frames and their order stay as they came.
pub fn redact_run(frames: &mut [Value]) {
    let mut values: Vec<(String, Vec<usize>, String)> = Vec::new();
    for (n, frame) in frames.iter().enumerate() {
        let Some((key, text)) = fragment(frame) else {
            continue;
        };
        match values.iter_mut().find(|(k, _, _)| *k == key) {
            Some((_, at, joined)) => {
                at.push(n);
                joined.push_str(text);
            }
            None => values.push((key, vec![n], text.to_string())),
        }
    }
    for (_, at, joined) in values {
        let redacted = super::redact(&joined);
        if redacted == joined {
            continue;
        }
        for (i, n) in at.into_iter().enumerate() {
            set(&mut frames[n], if i == 0 { &redacted } else { "" });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn piece(text: &str) -> Value {
        json!({ "type": "stream_event", "event": { "type": "content_block_delta", "index": 1,
            "delta": { "type": "input_json_delta", "partial_json": text } } })
    }

    #[test]
    fn a_key_split_across_fragments_is_redacted_whole() {
        let mut run = vec![
            piece("{\"key\":\"sk-pr"),
            piece("oj-0123456789abcdefghijklmnop\"}"),
            json!({ "method": "item/agentMessage/delta", "params": { "itemId": "m", "delta": "Bearer abc" } }),
            json!({ "method": "item/agentMessage/delta", "params": { "itemId": "m", "delta": "123def" } }),
        ];
        redact_run(&mut run);
        let stored = serde_json::to_string(&run).unwrap();
        for leak in ["sk-pr", "oj-0123", "abc", "123def"] {
            assert!(!stored.contains(leak), "{leak} is stored: {stored}");
        }
        assert_eq!(
            run[0]["event"]["delta"]["partial_json"],
            "{\"key\":\"[redacted:key]\"}"
        );
        assert_eq!(run[1]["event"]["delta"]["partial_json"], "");
    }

    #[test]
    fn fragments_with_no_secret_stay_as_they_came() {
        let mut run = vec![piece("{\"file_path\": \"no"), piece("te.txt\"}")];
        let before = run.clone();
        redact_run(&mut run);
        assert_eq!(run, before);
    }
}
