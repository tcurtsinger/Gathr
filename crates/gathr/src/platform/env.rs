//! The environment a new sign-in would give, for the engines and git (G23, 0028).
//!
//! A process keeps the environment it started with. Gathr runs for days in the tray, so a tool
//! installed since (rustup, node) isn't on its PATH, and its agents can't find `cargo`. On
//! Windows the person's variables are read again from the registry, as Explorer does when they
//! change; on macOS an app started from the Dock has launchd's short PATH, so the login shell is
//! asked for its own. Either way, what the service's PATH has that they don't is kept after them.

use std::ffi::OsString;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub type Vars = Vec<(OsString, OsString)>;

/// The environment, where its PATH came from, and why it's the service's own when it is.
pub struct Fresh {
    pub vars: Vars,
    pub from: &'static str,
    pub why: Option<String>,
}

#[cfg(windows)]
pub fn fresh(inherited: Vars) -> Fresh {
    const MACHINE: &str = r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
    const USER: &str = r"HKCU\Environment";
    match (registry(MACHINE), registry(USER)) {
        (Ok(machine), Ok(user)) => Fresh {
            vars: merged(inherited, &machine, &user),
            from: "registry",
            why: None,
        },
        (Err(why), _) | (_, Err(why)) => Fresh {
            vars: inherited,
            from: "inherited",
            why: Some(why),
        },
    }
}

#[cfg(target_os = "macos")]
pub fn fresh(mut inherited: Vars) -> Fresh {
    match login_path() {
        Ok(login) => {
            let old = get(&inherited, "PATH", false);
            set(&mut inherited, "PATH", union(&[&login, &old], ':'), false);
            Fresh {
                vars: inherited,
                from: "login shell",
                why: None,
            }
        }
        Err(why) => Fresh {
            vars: inherited,
            from: "inherited",
            why: Some(why),
        },
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn fresh(inherited: Vars) -> Fresh {
    Fresh {
        vars: inherited,
        from: "inherited",
        why: None,
    }
}

/// The login shell's PATH, as Terminal would have it: `-i` too, since nvm and many setups add to
/// it only in `.zshrc`. Whatever else the shell prints is skipped.
#[cfg(target_os = "macos")]
pub fn login_path() -> Result<String, String> {
    const MARK: &str = "__GATHR_PATH__";
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/zsh".into());
    let mut command = Command::new(&shell);
    command.args(["-ilc", &format!("printf '\\n%s%s\\n' {MARK} \"$PATH\"")]);
    let out = run(command, Duration::from_secs(5))?;
    let text = String::from_utf8_lossy(&out);
    text.lines()
        .find_map(|line| line.strip_prefix(MARK))
        .map(str::to_string)
        .ok_or_else(|| format!("{} didn't say its PATH", shell.to_string_lossy()))
}

/// One key's variables: name, value, and whether it's expanded (`REG_EXPAND_SZ`).
#[cfg(windows)]
fn registry(key: &str) -> Result<Vec<(String, String, bool)>, String> {
    Ok(parse_reg(&export(key)?))
}

/// A registry key as `reg export` writes it: UTF-16, where `reg query` prints in a code page that
/// varies by computer.
#[cfg(windows)]
pub(super) fn export(key: &str) -> Result<Vec<u8>, String> {
    let file = std::env::temp_dir().join(format!("gathr-reg-{}.reg", crate::ids::span_id()));
    let mut command = reg();
    command.args(["export", key]).arg(&file).arg("/y");
    let ran = run(command, Duration::from_secs(10));
    let read = std::fs::read(&file);
    let _ = std::fs::remove_file(&file);
    ran?;
    read.map_err(|e| format!("{key} wasn't read: {e}"))
}

/// `reg.exe` by its full path, so the PATH can't swap it.
#[cfg(windows)]
pub(super) fn reg() -> Command {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    Command::new(std::path::PathBuf::from(root).join(r"System32\reg.exe"))
}

/// Runs a short command with no input: its output, or why not, killed past `limit`. Only for
/// commands that change nothing (a shell printing its PATH, `reg export`) or Gathr's own entry at
/// login (`reg import`, opening System Settings).
pub(super) fn run(mut command: Command, limit: Duration) -> Result<Vec<u8>, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    super::no_window(&mut command);
    let name = command.get_program().to_string_lossy().into_owned();
    let mut child = command
        .spawn()
        .map_err(|e| format!("{name} didn't start: {e}"))?;
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{name} didn't answer in {} s", limit.as_secs()));
            }
            Err(e) => return Err(format!("{name} wasn't waited for: {e}")),
        }
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("{name}: {e}"))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("{name} failed ({}): {said}", out.status));
    }
    Ok(out.stdout)
}

/// The variables in a `reg export` file: quoted strings and `hex(2)` (expandable) values.
#[cfg(any(windows, test))]
pub fn parse_reg(bytes: &[u8]) -> Vec<(String, String, bool)> {
    let text = utf16le(bytes);
    let mut entries = Vec::new();
    let mut line = String::new();
    for part in text.trim_start_matches('\u{feff}').lines() {
        match part.strip_suffix('\\') {
            Some(head) => line.push_str(head.trim_start()),
            None => {
                line.push_str(part.trim_start());
                entries.extend(entry(&std::mem::take(&mut line)));
            }
        }
    }
    entries
}

#[cfg(any(windows, test))]
fn entry(line: &str) -> Option<(String, String, bool)> {
    let (name, rest) = quoted(line)?;
    let value = rest.strip_prefix('=')?;
    if value.starts_with('"') {
        return Some((name, quoted(value)?.0, false));
    }
    let bytes: Vec<u8> = value
        .strip_prefix("hex(2):")?
        .split(',')
        .filter_map(|b| u8::from_str_radix(b.trim(), 16).ok())
        .collect();
    let value = utf16le(&bytes);
    Some((name, value.trim_end_matches('\0').to_string(), true))
}

#[cfg(any(windows, test))]
pub(super) fn utf16le(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    String::from_utf16_lossy(&units)
}

/// A `"…"` string with `\\` and `\"` escapes at the start of `text`, and what follows it.
#[cfg(any(windows, test))]
fn quoted(text: &str) -> Option<(String, &str)> {
    let body = text.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = body.char_indices();
    while let Some((at, c)) = chars.next() {
        match c {
            '\\' => out.extend(chars.next().map(|(_, n)| n)),
            '"' => return Some((out, &body[at + 1..])),
            _ => out.push(c),
        }
    }
    None
}

/// The service's variables with the machine's and then the person's from the registry over them;
/// PATH is the machine's, then the person's, then the service's own entries they lack.
#[cfg(any(windows, test))]
pub fn merged(
    mut vars: Vars,
    machine: &[(String, String, bool)],
    user: &[(String, String, bool)],
) -> Vars {
    let mut paths = Vec::new();
    for (name, value, expands) in machine.iter().chain(user) {
        let value = match expands {
            true => expand(value, &vars),
            false => value.clone(),
        };
        match name.eq_ignore_ascii_case("path") {
            true => paths.push(value),
            false => set(&mut vars, name, value, true),
        }
    }
    paths.push(get(&vars, "PATH", true));
    let parts: Vec<&str> = paths.iter().map(String::as_str).collect();
    set(&mut vars, "Path", union(&parts, ';'), true);
    vars
}

/// `%NAME%` replaced by its value; a name with none stays as written, as Windows leaves it.
#[cfg(any(windows, test))]
fn expand(value: &str, vars: &Vars) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        let Some(len) = rest[start + 1..].find('%') else {
            break;
        };
        let name = &rest[start + 1..start + 1 + len];
        out.push_str(&rest[..start]);
        match find(vars, name, true) {
            Some(at) if !name.is_empty() => out.push_str(&vars[at].1.to_string_lossy()),
            _ => out.push_str(&rest[start..start + len + 2]),
        }
        rest = &rest[start + len + 2..];
    }
    out.push_str(rest);
    out
}

/// The entries of each list in order, each folder once (ignoring case and a trailing slash on
/// Windows), empty ones dropped.
fn union(lists: &[&str], separator: char) -> String {
    let key = |entry: &str| {
        let entry = entry.trim_end_matches(['\\', '/']);
        match separator == ';' {
            true => entry.to_lowercase(),
            false => entry.to_string(),
        }
    };
    let mut seen = std::collections::HashSet::new();
    let entries: Vec<&str> = lists
        .iter()
        .flat_map(|list| list.split(separator))
        .filter(|entry| !entry.is_empty() && seen.insert(key(entry)))
        .collect();
    entries.join(&separator.to_string())
}

/// A variable's place; names ignore case on Windows.
fn find(vars: &Vars, name: &str, ignore_case: bool) -> Option<usize> {
    vars.iter().position(|(n, _)| match ignore_case {
        true => n.to_string_lossy().eq_ignore_ascii_case(name),
        false => n == name,
    })
}

fn get(vars: &Vars, name: &str, ignore_case: bool) -> String {
    let at = find(vars, name, ignore_case);
    at.map_or_else(String::new, |at| vars[at].1.to_string_lossy().into_owned())
}

fn set(vars: &mut Vars, name: &str, value: String, ignore_case: bool) {
    match find(vars, name, ignore_case) {
        Some(at) => vars[at].1 = value.into(),
        None => vars.push((name.into(), value.into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UTF-16 with its byte-order mark, as `reg export` writes it.
    fn utf16(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        bytes
    }

    fn hex2(value: &str) -> String {
        let bytes: Vec<String> = value
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .map(|b| format!("{b:02x}"))
            .collect();
        // Long values wrap at about 80 columns with a trailing backslash.
        let (head, tail) = bytes.split_at(bytes.len() / 2);
        format!("hex(2):{},\\\r\n  {}", head.join(","), tail.join(","))
    }

    #[test]
    fn a_reg_export_reads_as_its_variables() {
        let file = format!(
            "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\Environment]\r\n\
             \"Path\"={}\r\n\"CARGO_HOME\"=\"X:\\\\packages\\\\cargo\"\r\n\"Quote\"=\"say \\\"hi\\\"\"\r\n\
             \"Count\"=dword:00000001\r\n\r\n",
            hex2(r"%USERPROFILE%\.cargo\bin;C:\Josée\bin")
        );
        assert_eq!(
            parse_reg(&utf16(&file)),
            [
                (
                    "Path".into(),
                    r"%USERPROFILE%\.cargo\bin;C:\Josée\bin".into(),
                    true
                ),
                ("CARGO_HOME".into(), r"X:\packages\cargo".into(), false),
                ("Quote".into(), r#"say "hi""#.into(), false),
            ]
        );
    }

    fn var(vars: &Vars, name: &str) -> String {
        get(vars, name, true)
    }

    #[test]
    fn the_registry_comes_first_and_the_services_own_path_entries_stay() {
        let inherited: Vars = vec![
            ("USERPROFILE".into(), r"C:\Users\Sam".into()),
            ("PATH".into(), r"C:\Windows;C:\dev\tools;".into()),
            ("CARGO_HOME".into(), r"C:\old".into()),
        ];
        let machine = [(
            "Path".into(),
            r"C:\Windows\;C:\Program Files\Git\cmd".into(),
            true,
        )];
        let user = [
            (
                "Path".into(),
                r"%USERPROFILE%\.cargo\bin;%NOPE%\bin".into(),
                true,
            ),
            ("CARGO_HOME".into(), r"%USERPROFILE%\.cargo".into(), true),
            ("RUSTUP_HOME".into(), r"R:\rustup".into(), false),
        ];
        let vars = merged(inherited, &machine, &user);
        assert_eq!(
            var(&vars, "path"),
            r"C:\Windows\;C:\Program Files\Git\cmd;C:\Users\Sam\.cargo\bin;%NOPE%\bin;C:\dev\tools"
        );
        assert_eq!(var(&vars, "CARGO_HOME"), r"C:\Users\Sam\.cargo");
        assert_eq!(var(&vars, "RUSTUP_HOME"), r"R:\rustup");
        let paths = vars.iter().filter(|(n, _)| n.eq_ignore_ascii_case("path"));
        assert_eq!(paths.count(), 1, "one PATH: {vars:?}");
    }
}
