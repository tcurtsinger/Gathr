//! A Codex account's login state, read from its folder without starting Codex (0002, 0033).
//! Codex renews its own tokens and guards the file itself (`auth.lock`), so only presence is read.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::claude::login::Login;

/// `auth.json` as Codex keeps it with `cli_auth_credentials_store="file"`. Only whether each
/// credential is there is kept: the values are dropped as they're read.
#[derive(Deserialize)]
struct File {
    #[serde(rename = "OPENAI_API_KEY", default)]
    key: Present,
    tokens: Option<Tokens>,
}

#[derive(Deserialize)]
struct Tokens {
    #[serde(default)]
    refresh_token: Present,
}

#[derive(Default)]
struct Present(bool);

impl<'de> Deserialize<'de> for Present {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Option::<String>::deserialize(d)?;
        Ok(Present(value.is_some_and(|v| !v.is_empty())))
    }
}

/// The login file in an account's Codex folder (its `CODEX_HOME`).
pub fn credentials(folder: &Path) -> PathBuf {
    folder.join("auth.json")
}

/// Signed in when the file holds a refresh token or a key; signed out when it's missing.
pub fn status(folder: &Path) -> Login {
    let text = match std::fs::read_to_string(credentials(folder)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Login::SignedOut,
        Err(_) => return Login::Unknown,
    };
    match serde_json::from_str::<File>(&text) {
        Ok(file) if file.key.0 || file.tokens.as_ref().is_some_and(|t| t.refresh_token.0) => {
            Login::SignedIn
        }
        Ok(_) => Login::SignedOut,
        Err(_) => Login::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(text: Option<&str>) -> Login {
        let dir = std::env::temp_dir().join(format!("gathr-codex-login-{}", crate::ids::uuid()));
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(text) = text {
            std::fs::write(credentials(&dir), text).unwrap();
        }
        let login = status(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        login
    }

    #[test]
    fn signed_in_needs_a_refresh_token_or_a_key() {
        assert_eq!(with(None), Login::SignedOut);
        let chatgpt = r#"{"OPENAI_API_KEY":null,"tokens":{"id_token":"a","access_token":"b","refresh_token":"c"}}"#;
        assert_eq!(with(Some(chatgpt)), Login::SignedIn);
        assert_eq!(with(Some(r#"{"OPENAI_API_KEY":"sk-x"}"#)), Login::SignedIn);
        assert_eq!(
            with(Some(r#"{"OPENAI_API_KEY":null,"tokens":null}"#)),
            Login::SignedOut
        );
        assert_eq!(with(Some("{\"OPENAI_API")), Login::Unknown);
    }
}
