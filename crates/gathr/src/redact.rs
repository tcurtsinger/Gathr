//! Removes secrets from text before it's written anywhere (docs/decisions/0011-event-record.md).
//! Each removal leaves `[redacted:<kind>]`.

pub mod stream;

/// Names whose value is a secret, matched word by word in a flag (`--api-key`, `-AccessToken`), a
/// header (`X-Auth-Token`) or a PowerShell variable (`$env:GH_TOKEN`); never inside a longer word
/// (`--keyboard`, `--max-tokens`). `--pwd` is left out: it's as often a folder.
const SECRET_WORDS: &[&str] = &[
    "token",
    "key",
    "secret",
    "password",
    "passwd",
    "auth",
    "credential",
    "credentials",
];

/// What a secret's name ends in when its words are run together (`accesstoken`, `clientsecret`).
const SECRET_ENDS: &[&str] = &["token", "secret", "password", "passwd"];

/// `key` ends too many plain words (`monkey`, `hotkey`) to count that way: only after one of these.
const KEY_OF: &[&str] = &["api", "access", "private", "secret", "auth"];

/// Flags named like a secret that take no value: the word after them is not one.
const NO_VALUE: &[&str] = &["password-stdin", "with-token"];

/// How a provider's key starts, when the word is at least 20 characters.
const KEY_PREFIXES: &[&str] = &[
    "sk-",
    "sk_live_",
    "sk_test_",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
];

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

/// How every mark left for a secret starts.
pub(crate) const MARK: &str = "[redacted:";

/// Where a word ends: whitespace, or the punctuation JSON and shell quoting put around a value,
/// so a key inside `{"key":"sk-…"}` or a fragment of streamed tool input is still one word.
fn ends_word(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '"' | '\'' | '{' | '}' | '[' | ']' | '(' | ')' | ',' | ';' | ':' | '`' | '\\'
        )
}

/// Ends a command: a secret waited for after a flag isn't looked for past it.
fn ends_command(c: char) -> bool {
    matches!(c, ';' | '|' | '&' | '<' | '>' | ')')
}

/// One pass, left to right. What's already hidden stays as it is, so text redacted twice (a
/// step's text, then a frame that carries it) reads the same: a mark is passed over whole, and
/// stands for the secret a `--token` or `Bearer` before it was waiting for.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut waiting: Option<&'static str> = None;
    let mut rest = text;
    while let Some(first) = rest.chars().next() {
        if let Some(len) = mark_len(rest) {
            waiting = None;
            out.push_str(&rest[..len]);
            rest = &rest[len..];
            continue;
        }
        if let Some(kind) = waiting {
            // A quote opens a value only after a gap: straight after the flag it closes the
            // flag's own quotes (`"--token","abc"`), and what follows is still waited for.
            let opens = out.ends_with(|c: char| c.is_whitespace() || c == '=' || c == ':');
            let len = match first {
                '"' | '\'' if !opens => 0,
                '[' | '{' | '(' | ',' | ':' | '=' | '}' | ']' => 0,
                c if ends_command(c) => 0,
                _ => value_len(rest),
            };
            if len == 0 {
                // In raw JSON text the wait ends where the JSON closes: at its brackets, and
                // at the quote that ends the string the flag was the last word of.
                let closes = matches!(first, '}' | ']') || (opens && closes_json(rest));
                if ends_command(first) || closes {
                    waiting = None;
                }
                out.push(first);
                rest = &rest[first.len_utf8()..];
            } else {
                waiting = None;
                push_hidden(&mut out, &rest[..len], kind);
                rest = &rest[len..];
            }
            continue;
        }
        let end = rest.find(ends_word).unwrap_or(rest.len());
        if end == 0 {
            out.push(first);
            rest = &rest[first.len_utf8()..];
            continue;
        }
        let taken = word(rest, end, &mut out, &mut waiting);
        rest = &rest[taken..];
    }
    out
}

/// The length of the `[redacted:<kind>]` that `text` starts with.
fn mark_len(text: &str) -> Option<usize> {
    let kind = text.strip_prefix(MARK)?;
    let len = kind.bytes().take(17).position(|b| b == b']')?;
    let named = len > 0 && kind[..len].bytes().all(|b| b.is_ascii_lowercase());
    named.then_some(MARK.len() + len + 1)
}

/// How much of `text`, from its start, is one value: a quoted run, or up to whitespace, a quote,
/// `;` or `)`. 0 when it starts with none, or with a mark.
fn value_len(text: &str) -> usize {
    if text.starts_with(MARK) {
        return 0;
    }
    match text.bytes().next() {
        Some(b'"' | b'\'') if closes_json(text) => 0,
        // A failed search means no such quote is left, so none is searched for again: one pass.
        Some(quote @ (b'"' | b'\'')) => {
            let close = text.bytes().skip(1).position(|b| b == quote);
            close.map_or(0, |at| at + 2)
        }
        _ => {
            let ends =
                |c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '\\' | ';' | ')');
            text.find(ends).unwrap_or(text.len())
        }
    }
}

/// Whether the quote `text` starts with closes a string of raw JSON text rather than opening a
/// value: the text ends there, or goes on as JSON does, with `}`, `]`, or `,` or `:` and the next
/// JSON value. So `"Bearer ","path":…` hides nothing, and a shell's `--token ":abc"` still does.
fn closes_json(text: &str) -> bool {
    let after = &text[1..];
    let goes_on = after.strip_prefix([',', ':']).is_some_and(|next| {
        let next = next.trim_start();
        next.starts_with(['"', '{', '[', '-'])
            || next.starts_with(|c: char| c.is_ascii_digit())
            || ["true", "false", "null"]
                .iter()
                .any(|word| next.starts_with(word))
    });
    after.is_empty() || after.starts_with(['}', ']']) || goes_on
}

/// The mark in place of `value`, inside the value's own quotes when it has them, so JSON text
/// and a quoted command keep their shape.
fn push_hidden(out: &mut String, value: &str, kind: &str) {
    let quote = value.chars().next().filter(|c| matches!(c, '"' | '\''));
    out.extend(quote);
    out.push_str(&format!("{MARK}{kind}]"));
    out.extend(quote);
}

/// Writes `rest[..upto]`, then hides the value that starts there; with none there yet (a space,
/// an open quote), the secret is waited for. Answers how much of `rest` that took.
fn hide(
    rest: &str,
    upto: usize,
    kind: &'static str,
    out: &mut String,
    waiting: &mut Option<&'static str>,
) -> usize {
    out.push_str(&rest[..upto]);
    let len = value_len(&rest[upto..]);
    match len {
        0 => *waiting = Some(kind),
        _ => push_hidden(out, &rest[upto..upto + len], kind),
    }
    upto + len
}

/// Whether a name says its value is a secret: one of its words, split at `-`, `_`, `.` and where
/// a capital follows a small letter (`ApiKey`), is a secret word, ends in one (`accesstoken`) or
/// is a kind of key (`apikey`); or it is `pass` whole.
fn secret_name(name: &str) -> bool {
    let mut words = String::with_capacity(name.len() + 4);
    let mut small = false;
    for c in name.chars() {
        if c.is_ascii_uppercase() && small {
            words.push('-');
        }
        small = c.is_ascii_lowercase() || c.is_ascii_digit();
        words.push(c.to_ascii_lowercase());
    }
    let secret = |word: &str| {
        SECRET_WORDS.contains(&word)
            || SECRET_ENDS.iter().any(|end| word.ends_with(end))
            || word
                .strip_suffix("key")
                .is_some_and(|of| KEY_OF.contains(&of))
    };
    words.split(['-', '_', '.']).any(secret) || name.eq_ignore_ascii_case("pass")
}

/// Reads the word `rest[..end]` with what follows it, writes it with its secret hidden, and
/// answers how much of `rest` that took.
fn word(rest: &str, end: usize, out: &mut String, waiting: &mut Option<&'static str>) -> usize {
    let (word, after) = rest.split_at(end);
    let named = word.trim_start_matches('-');
    let dashes = word.len() - named.len();
    let name = named.split('=').next().unwrap_or(named);
    let flag = (1..=2).contains(&dashes) && name.starts_with(|c: char| c.is_ascii_alphabetic());
    let alone = !word.contains('=') && NO_VALUE.contains(&name.to_ascii_lowercase().as_str());
    if flag && secret_name(name) && !alone {
        // PowerShell also takes `-Token:value`.
        let colon = after.starts_with(':') && !after[1..].starts_with(char::is_whitespace);
        return match word.find('=') {
            Some(eq) => hide(rest, eq + 1, "flag", out, waiting),
            None if colon => hide(rest, end + 1, "flag", out, waiting),
            None => {
                out.push_str(word);
                *waiting = Some("flag");
                end
            }
        };
    }
    // Before the URL: a secret flag's or an environment value that is a URL goes whole.
    if let Some((name, _)) = word.split_once('=') {
        let env_name = name.len() >= 2
            && name.starts_with(|c: char| c.is_ascii_uppercase())
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if env_name && value_len(&rest[name.len() + 1..]) > 0 {
            return hide(rest, name.len() + 1, "env", out, waiting);
        }
    }
    if let Some((at, len)) = url_login(rest, end) {
        out.push_str(&rest[..at]);
        push_hidden(out, "", "url");
        return at + len;
    }
    if word.eq_ignore_ascii_case("$env")
        && let Some(value) = powershell_env(after)
    {
        return hide(rest, end + value, "env", out, waiting);
    }
    if let Some(value) = header(word, after) {
        return hide(rest, end + value, "header", out, waiting);
    }
    if word.eq_ignore_ascii_case("bearer") {
        out.push_str(word);
        *waiting = Some("bearer");
        return end;
    }
    if let Some(kind) = key_kind(word) {
        push_hidden(out, "", kind);
        return end;
    }
    if let Some((name, value)) = word.split_once('=')
        && let Some(kind) = key_kind(value)
    {
        out.push_str(&rest[..=name.len()]);
        push_hidden(out, "", kind);
        return end;
    }
    out.push_str(word);
    end
}

/// `scheme://user:password@host`: where the password starts in `rest` and how long it is, the
/// name kept; or a key standing alone as the login (`https://ghp_…@host`).
fn url_login(rest: &str, scheme_end: usize) -> Option<(usize, usize)> {
    let (word, after) = rest.split_at(scheme_end);
    let host = after.strip_prefix("://")?;
    // The scheme is the end of the word: `--index-url=https`, `<https`.
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.');
    let scheme = word.rsplit(|c: char| !plain(c)).next()?;
    if !scheme.contains(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let ends =
        |c: char| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '"' | '\'' | '`' | '\\');
    let host = &host[..host.find(ends).unwrap_or(host.len())];
    let login = &host[..host.rfind('@')?];
    let start = scheme_end + "://".len();
    let secret = match login.find(':') {
        Some(colon) => colon + 1,
        None => key_kind(login).map(|_| 0)?,
    };
    let hidden = login[secret..].is_empty() || login[secret..].starts_with(MARK);
    (!hidden).then_some((start + secret, login.len() - secret))
}

/// `$env:NAME = value` with a secret's name: how far after `$env` the value starts.
fn powershell_env(after: &str) -> Option<usize> {
    let name = after.strip_prefix(':')?;
    let named = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let name_len = name.find(|c: char| !named(c)).unwrap_or(name.len());
    let assigned = name[name_len..].trim_start_matches([' ', '\t']);
    let value = assigned.strip_prefix('=')?.trim_start_matches([' ', '\t']);
    secret_name(&name[..name_len]).then_some(after.len() - value.len())
}

/// A header in plain text, `Name: value`: how far after the name the secret starts. A hyphened
/// name with a secret word (`X-Api-Key`), `apikey`, or `Authorization` with any scheme but
/// Bearer, which `Bearer` itself hides. A lone word before a colon is prose (`key: value`).
fn header(name: &str, after: &str) -> Option<usize> {
    let value = after.strip_prefix(':')?;
    let lower = name.to_ascii_lowercase();
    let authorization = lower == "authorization" || lower.ends_with("-authorization");
    // curl takes `Name:value` too. Only names that can't be prose are read with no space
    // (`re-auth:needed` is prose), and never a URL's `x-scheme://`.
    let tight =
        (authorization || lower.starts_with("x-")) && !value.is_empty() && !value.starts_with('/');
    if !value.starts_with([' ', '\t']) && !tight {
        return None;
    }
    let value = value.trim_start_matches([' ', '\t']);
    let at = after.len() - value.len();
    if authorization {
        let scheme = value_len(value);
        let named = scheme > 0 && value[..scheme].chars().all(|c| c.is_ascii_alphabetic());
        let then = value[scheme..].strip_prefix(' ');
        let given = then.is_some_and(|t| t.starts_with(MARK) || value_len(t) > 0);
        let bearer = value[..scheme].eq_ignore_ascii_case("bearer");
        return match (named && given, bearer) {
            (_, true) => None,
            (true, false) => Some(at + scheme + 1),
            (false, false) => Some(at),
        };
    }
    let listed = lower == "apikey" || (name.contains('-') && secret_name(name));
    (listed && !name.starts_with('-')).then_some(at)
}

/// The kind of secret a whole word is, by its shape: a provider's key or a JWT.
fn key_kind(word: &str) -> Option<&'static str> {
    let prefixed = word.len() >= 20 && KEY_PREFIXES.iter().any(|p| word.starts_with(p));
    let capital = |b: u8| b.is_ascii_uppercase() || b.is_ascii_digit();
    let aws = word.len() == 20 && word.starts_with("AKIA") && word.bytes().all(capital);
    match () {
        _ if prefixed || aws => Some("key"),
        _ if is_jwt(word) => Some("jwt"),
        _ => None,
    }
}

/// Every string in a JSON value redacted, and a secret-named key's string hidden whole. Frames
/// and event attributes go through here, not `redact` on their text: in JSON text a secret sits
/// inside quotes and escapes that `redact` alone can't see past.
pub fn redact_json(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    let secret_key = |k: &str| {
        // `X-Api-Key` is `api_key` too.
        let k = k.to_ascii_lowercase().replace('-', "_");
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
mod tests;
