//! Open at login: Gathr's window starts in the tray when the person signs in to this computer. A
//! value in their Run key on Windows, a LaunchAgent on macOS: both theirs alone, with no admin.

use std::path::Path;

/// Gathr's entry at login for one data folder.
pub struct Item {
    name: String,
    args: Vec<String>,
}

#[cfg(not(target_os = "macos"))]
const NAME: &str = "Gathr";
#[cfg(target_os = "macos")]
const NAME: &str = "io.github.tcurtsinger.gathr";

impl Item {
    /// The window `app` on the data folder: the person's own folder under the plain name, a test
    /// folder under its key and with its `--data`, so turning a test folder's off leaves theirs.
    pub fn new(app: &Path, data: &Path) -> Item {
        let key = crate::launch::folder_key(data, &super::default_data_dir());
        let mut args = vec![app.to_string_lossy().into_owned(), "--hidden".into()];
        if key.is_some() {
            let data = std::path::absolute(data).unwrap_or_else(|_| data.to_path_buf());
            args.extend(["--data".into(), data.to_string_lossy().into_owned()]);
        }
        Item {
            name: format!("{NAME}{}", key.unwrap_or_default()),
            args,
        }
    }
}

#[cfg(windows)]
const RUN: &str = r"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run";
/// Where Task Manager and Settings > Apps > Startup turn an entry off, leaving it in Run.
#[cfg(windows)]
const APPROVED: &str =
    r"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

/// Whether Gathr starts at login as this item: listed with this command and not turned off.
#[cfg(windows)]
pub fn is_on(item: &Item) -> Result<bool, String> {
    use super::env;
    let command = command_line(&item.args);
    let listed = entries(RUN, env::export(RUN))?
        .iter()
        .any(|(name, value, _)| name.eq_ignore_ascii_case(&item.name) && *value == command);
    // A person who never turned anything off may have no such key.
    let off = env::export(APPROVED).is_ok_and(|b| turned_off(&env::utf16le(&b), &item.name));
    Ok(listed && !off)
}

/// The values in `key`, from a first `reg export` of it. A profile nothing ever started at login
/// from may have no Run key (CI's runners): none. `reg` says so only in the system's language, so
/// the key's parent is asked instead, and a key that appeared meanwhile is read again (the likeliest
/// cause of CI's rare "unable to find" on a fresh runner: something else made the key between the two).
#[cfg(windows)]
fn entries(
    key: &str,
    first: Result<Vec<u8>, String>,
) -> Result<Vec<(String, String, bool)>, String> {
    use super::env;
    let bytes = match first {
        Ok(bytes) => bytes,
        Err(_) if !key_exists(key)? => return Ok(Vec::new()),
        Err(_) => env::export(key)?,
    };
    Ok(env::parse_reg(&bytes))
}

/// Whether `key` is one of its parent's subkeys, as `reg query` lists them (key names print as
/// they are in any language).
#[cfg(windows)]
fn key_exists(key: &str) -> Result<bool, String> {
    use super::env;
    let (parent, _) = key.rsplit_once('\\').expect("a subkey");
    let mut command = env::reg();
    command.args(["query", parent]);
    let listed = env::run(command, std::time::Duration::from_secs(10))
        .map_err(|e| format!("{parent} wasn't listed: {e}"))?;
    let listed = String::from_utf8_lossy(&listed);
    Ok(listed.lines().any(|l| l.trim().eq_ignore_ascii_case(key)))
}

/// Adds or removes the item, and clears any earlier "off" so that on means on.
#[cfg(windows)]
pub fn set(item: &Item, on: bool) -> Result<(), String> {
    use super::env;
    let command = on.then(|| command_line(&item.args));
    let text = reg_file(RUN, APPROVED, &item.name, command.as_deref());
    // `reg import` reads UTF-16 with its mark, which keeps any path's letters as they are.
    let bytes: Vec<u8> = format!("\u{feff}{text}")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let file = std::env::temp_dir().join(format!("gathr-login-{}.reg", crate::ids::span_id()));
    std::fs::write(&file, bytes).map_err(|e| format!("{} wasn't written: {e}", file.display()))?;
    let mut command = env::reg();
    command.arg("import").arg(&file);
    let ran = env::run(command, std::time::Duration::from_secs(10));
    let _ = std::fs::remove_file(&file);
    ran.map(|_| ())
}

/// Windows' command line for the arguments: each one quoted (no Windows path holds a `"`).
#[cfg(any(windows, test))]
fn command_line(args: &[String]) -> String {
    let quoted: Vec<String> = args.iter().map(|a| format!("\"{a}\"")).collect();
    quoted.join(" ")
}

/// A `.reg` file setting `name` in `run` to `command`, or deleting it with none, and deleting it
/// from `approved` either way.
#[cfg(any(windows, test))]
fn reg_file(run: &str, approved: &str, name: &str, command: Option<&str>) -> String {
    let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let value = command.map_or("-".to_string(), |c| format!("\"{}\"", escape(c)));
    let name = escape(name);
    format!(
        "Windows Registry Editor Version 5.00\r\n\r\n[{run}]\r\n\"{name}\"={value}\r\n\r\n\
         [{approved}]\r\n\"{name}\"=-\r\n"
    )
}

/// Whether an export of StartupApproved turns `name` off: its first byte is odd (02 on, 03 off).
#[cfg(any(windows, test))]
fn turned_off(export: &str, name: &str) -> bool {
    let head = format!("\"{name}\"=hex:").to_lowercase();
    export.lines().any(|line| {
        let line = line.to_lowercase();
        let first = line.strip_prefix(&head).and_then(|bytes| bytes.get(..2));
        first
            .and_then(|b| u8::from_str_radix(b, 16).ok())
            .is_some_and(|b| b & 1 == 1)
    })
}

/// Whether Gathr starts at login as this item: its LaunchAgent is there, as this build writes it,
/// and not turned off in System Settings.
#[cfg(target_os = "macos")]
pub fn is_on(item: &Item) -> Result<bool, String> {
    let file = agent_file(item)?;
    let listed = match std::fs::read_to_string(&file) {
        Ok(text) => text == plist(&item.name, &item.args),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(format!("{} wasn't read: {e}", file.display())),
    };
    Ok(listed && !turned_off_in_system_settings(&file)?)
}

/// Writes or removes the item's LaunchAgent. It takes effect at the next login.
#[cfg(target_os = "macos")]
pub fn set(item: &Item, on: bool) -> Result<(), String> {
    let file = agent_file(item)?;
    let done = match on {
        true => file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&file, plist(&item.name, &item.args))),
        false => std::fs::remove_file(&file).or_else(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(e),
        }),
    };
    done.map_err(|e| format!("{} wasn't changed: {e}", file.display()))?;
    if on && turned_off_in_system_settings(&file)? {
        // macOS keeps the person's "off" even for a file written anew, and only they can undo it.
        service_management(&["open"])?;
        return Err("Turned off in System Settings > Login Items".into());
    }
    Ok(())
}

/// Whether the person turned the LaunchAgent off in System Settings > Login Items. macOS keeps
/// that in its own record of background items, not in launchd's disabled list.
#[cfg(target_os = "macos")]
fn turned_off_in_system_settings(file: &Path) -> Result<bool, String> {
    let status = service_management(&["status", &file.to_string_lossy()])?;
    Ok(String::from_utf8_lossy(&status).trim() == "2")
}

/// Apple's ServiceManagement through `osascript`'s bridge, with no library or unsafe code. `status
/// <file>` prints SMAppService's status for a LaunchAgent (2 is the person's "off"; 0 before macOS
/// 13, which has no such switch); `open` opens System Settings > Login Items.
#[cfg(target_os = "macos")]
fn service_management(args: &[&str]) -> Result<Vec<u8>, String> {
    const SCRIPT: &str = "ObjC.import('ServiceManagement'); function run(a) { const s = $.SMAppService; if (!s) return 0; \
        if (a[0] === 'open') { s.openSystemSettingsLoginItems; return 0 } \
        return s.statusForLegacyURL($.NSURL.fileURLWithPath(a[1])) }";
    let mut command = std::process::Command::new("/usr/bin/osascript");
    command.args(["-l", "JavaScript", "-e", SCRIPT]).args(args);
    super::env::run(command, std::time::Duration::from_secs(5))
}

#[cfg(target_os = "macos")]
fn agent_file(item: &Item) -> Result<std::path::PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME isn't set")?;
    let file = format!("Library/LaunchAgents/{}.plist", item.name);
    Ok(std::path::PathBuf::from(home).join(file))
}

/// A LaunchAgent that runs `args` once at login.
#[cfg(any(target_os = "macos", test))]
fn plist(label: &str, args: &[String]) -> String {
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let args: String = args
        .iter()
        .map(|a| format!("\t\t<string>{}</string>\n", escape(a)))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n\
         \t<key>Label</key>\n\t<string>{}</string>\n\
         \t<key>ProgramArguments</key>\n\t<array>\n{args}\t</array>\n\
         \t<key>RunAtLoad</key>\n\t<true/>\n\
         </dict>\n</plist>\n",
        escape(label)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn a_key_is_found_among_its_parents_subkeys_and_a_missing_one_isnt() {
        assert_eq!(
            key_exists(r"HKEY_CURRENT_USER\Software\Microsoft"),
            Ok(true)
        );
        let missing = r"HKEY_CURRENT_USER\Software\Microsoft\GathrNoSuchKey";
        assert_eq!(key_exists(missing), Ok(false));
    }

    /// A made-up key under the person's own, deleted however the test ends.
    #[cfg(windows)]
    struct ScratchKey(String);

    #[cfg(windows)]
    impl Drop for ScratchKey {
        fn drop(&mut self) {
            let mut command = super::super::env::reg();
            command.args(["delete", &self.0, "/f"]);
            let _ = super::super::env::run(command, std::time::Duration::from_secs(10));
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_key_made_after_its_first_read_failed_is_read_again() {
        use super::super::env;
        let key = ScratchKey(format!(
            r"HKEY_CURRENT_USER\Software\GathrTest-{}",
            crate::ids::span_id()
        ));
        let mut command = env::reg();
        command.args([
            "add",
            &key.0,
            "/v",
            "Gathr",
            "/d",
            r"C:\Apps\gathr-app.exe",
            "/f",
        ]);
        env::run(command, std::time::Duration::from_secs(10)).unwrap();
        let missing =
            Err("ERROR: The system was unable to find the specified registry key or value.".into());
        let read = entries(&key.0, missing).unwrap();
        assert_eq!(
            read,
            [("Gathr".into(), r"C:\Apps\gathr-app.exe".into(), false)]
        );
    }

    #[test]
    fn a_reg_file_escapes_the_command_and_deletes_with_a_dash() {
        let command = command_line(&[
            r"C:\Program Files\Gathr\gathr-app.exe".into(),
            "--hidden".into(),
        ]);
        let on = reg_file(
            r"HKEY_CURRENT_USER\Run",
            r"HKEY_CURRENT_USER\Approved",
            "Gathr",
            Some(&command),
        );
        let expected = r#""Gathr"="\"C:\\Program Files\\Gathr\\gathr-app.exe\" \"--hidden\"""#;
        assert!(
            on.contains(&format!("[HKEY_CURRENT_USER\\Run]\r\n{expected}\r\n")),
            "{on}"
        );
        assert!(
            on.contains("[HKEY_CURRENT_USER\\Approved]\r\n\"Gathr\"=-\r\n"),
            "{on}"
        );
        let off = reg_file(
            r"HKEY_CURRENT_USER\Run",
            r"HKEY_CURRENT_USER\Approved",
            "Gathr",
            None,
        );
        assert!(
            off.contains("[HKEY_CURRENT_USER\\Run]\r\n\"Gathr\"=-\r\n"),
            "{off}"
        );
    }

    #[test]
    fn only_an_odd_first_byte_under_its_own_name_turns_an_entry_off() {
        let export = "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\…\\Run]\r\n\
            \"Gathr.data-1\"=hex:03,00,00,00,a1,b2,c3,d4,\\\r\n  e5,f6,07,08\r\n\
            \"Made-up app\"=hex:02,00,00,00,00,00,00,00,00,00,00,00\r\n\
            \"Other\"=hex:07,00,00,00,00,00,00,00,00,00,00,00\r\n";
        assert!(turned_off(export, "Gathr.data-1"));
        assert!(turned_off(export, "gathr.DATA-1"), "names ignore case");
        assert!(!turned_off(export, "Made-up app"));
        assert!(!turned_off(export, "Gathr"), "a name that is only a prefix");
        assert!(turned_off(export, "Other"));
    }

    /// Calls macOS's record of background items for real: it must answer, not crash, and a file it
    /// has never seen isn't off.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_launch_agent_macos_has_never_seen_isnt_turned_off() {
        let file = std::env::temp_dir().join(format!("gathr-{}.plist", crate::ids::span_id()));
        assert_eq!(turned_off_in_system_settings(&file), Ok(false));
    }

    #[test]
    fn a_plist_escapes_each_argument() {
        let text = plist(
            "io.example.app",
            &[
                "/Apps/A & B/gathr-app".into(),
                "--data".into(),
                "/x/<y>".into(),
            ],
        );
        assert!(
            text.contains("<string>/Apps/A &amp; B/gathr-app</string>"),
            "{text}"
        );
        assert!(text.contains("<string>/x/&lt;y&gt;</string>"), "{text}");
        assert!(text.contains("<key>RunAtLoad</key>\n\t<true/>"), "{text}");
    }

    #[test]
    fn a_test_folder_gets_its_own_name_and_data_and_the_persons_own_neither() {
        let app = Path::new("/Apps/gathr-app");
        let own = Item::new(app, &super::super::default_data_dir());
        assert_eq!(own.name, NAME);
        assert_eq!(own.args, ["/Apps/gathr-app", "--hidden"]);
        let test = Item::new(app, Path::new("/work/data/made-up"));
        assert!(
            test.name.starts_with(&format!("{NAME}.data-")),
            "{}",
            test.name
        );
        assert_eq!(test.args[2], "--data");
    }
}
