//! `--text`: any JSON answer as indented `key: value` lines.

use serde_json::Value;

pub fn render(value: &Value) -> String {
    let mut out = String::new();
    write(value, 0, &mut out);
    out.trim_end().to_string()
}

fn write(value: &Value, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match value {
        Value::Object(map) => {
            for (key, item) in map {
                if is_leaf(item) {
                    out.push_str(&format!("{pad}{key}: {}\n", leaf(item)));
                } else {
                    out.push_str(&format!("{pad}{key}:\n"));
                    write(item, depth + 1, out);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                if is_leaf(item) {
                    out.push_str(&format!("{pad}- {}\n", leaf(item)));
                } else {
                    out.push_str(&format!("{pad}-\n"));
                    write(item, depth + 1, out);
                }
            }
        }
        other => out.push_str(&format!("{pad}{}\n", leaf(other))),
    }
}

fn is_leaf(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => true,
    }
}

fn leaf(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
