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
    let listed = env::parse_reg(&env::export(RUN)?)
        .iter()
        .any(|(name, value, _)| name.eq_ignore_ascii_case(&item.name) && *value == command);
    // A person who never turned anything off may have no such key.
    let off = env::export(APPROVED).is_ok_and(|b| turned_off(&env::utf16le(&b), &item.name));
    Ok(listed && !off)
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
    Ok(listed && !turned_off_in_system_settings(&file))
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
    if on && turned_off_in_system_settings(&file) {
        // macOS keeps the person's "off" even for a file written anew, and only they can undo it.
        unsafe { objc2_service_management::SMAppService::openSystemSettingsLoginItems() };
        return Err("Turned off in System Settings > Login Items".into());
    }
    Ok(())
}

/// Whether the person turned the LaunchAgent off in System Settings > Login Items (macOS 13 on).
/// macOS keeps that in its own record of background items, not in launchd's disabled list.
#[cfg(target_os = "macos")]
fn turned_off_in_system_settings(file: &Path) -> bool {
    use objc2_service_management::{SMAppService, SMAppServiceStatus};
    if !objc2::available!(macos = 13.0) {
        return false;
    }
    let Some(url) = objc2_foundation::NSURL::from_file_path(file) else {
        return false;
    };
    let status = unsafe { SMAppService::statusForLegacyURL(&url) };
    status == SMAppServiceStatus::RequiresApproval
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
        assert!(!turned_off_in_system_settings(&file));
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
