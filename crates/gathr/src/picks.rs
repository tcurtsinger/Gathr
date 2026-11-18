//! The names the person picked from the window's `@` list, kept with their message so it draws them
//! blue (0028, 0072). A span counts UTF-16 code units of the text, as the window does.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// One picked `@Name`: where it starts, how long it is with its `@`, and whose it is.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pick {
    pub at: usize,
    pub len: usize,
    pub agent: String,
}

/// The picks as they're kept, in the order they stand in `text`, or none when there are none. Refused
/// when a span isn't inside the text, cuts a character in half, isn't `@` and a name, or overlaps another.
pub fn kept(text: &str, mut picks: Vec<Pick>) -> Result<Option<String>, &'static str> {
    if picks.is_empty() {
        return Ok(None);
    }
    let mut starts = HashMap::new();
    let mut end = 0;
    for c in text.chars() {
        starts.insert(end, c);
        end += c.len_utf16();
    }
    picks.sort_by_key(|p| p.at);
    let mut free = 0;
    for p in &picks {
        let stop = match p.at.checked_add(p.len) {
            Some(stop) if stop <= end => stop,
            _ => return Err("a picked name isn't inside the text"),
        };
        if p.at < free {
            return Err("two picked names overlap");
        }
        if !starts.contains_key(&p.at) || (stop < end && !starts.contains_key(&stop)) {
            return Err("a picked name cuts a character in half");
        }
        if starts.get(&p.at) != Some(&'@') {
            return Err("a picked name doesn't start with @");
        }
        if p.len < 2 {
            return Err("a picked name has no name after @");
        }
        free = stop;
    }
    Ok(Some(
        serde_json::to_string(&picks).expect("picks serialize"),
    ))
}
