//! A tool's key, sealed for this computer's user, never kept as text on disk: with DPAPI on Windows,
//! in the Keychain on a Mac (the database then holds only a marker). Any program running as the same
//! user can still unseal it, an agent in Full mode included: the window says no more than that the
//! key stays in Gathr.

use crate::Error;

/// What's stored for `key`, the key of the tool `id`.
pub fn seal(id: &str, key: &str) -> Result<Vec<u8>, Error> {
    imp::seal(id, key)
}

/// The key back from what `seal` stored for the tool `id`.
pub fn unseal(id: &str, sealed: &[u8]) -> Result<String, Error> {
    imp::unseal(id, sealed)
}

/// Forgets the tool's key where `seal` put it outside the database.
pub fn forget(id: &str) {
    imp::forget(id)
}

/// DPAPI for the current user, through Windows PowerShell's `ProtectedData`: the workspace forbids
/// unsafe code, which calling DPAPI directly needs. The key goes in on stdin and comes back on
/// stdout, as hex so no console code page can change it; never on a command line. An unsealed key
/// is kept in memory, as the service holds it anyway, so PowerShell starts once per key.
#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::sync::Mutex;

    use crate::Error;

    const SEAL: &str = "Add-Type -AssemblyName System.Security; $h = [Console]::In.ReadToEnd().Trim(); \
        $b = [byte[]]($h -split '(..)' -ne '' | ForEach-Object { [Convert]::ToByte($_, 16) }); \
        [Convert]::ToBase64String([Security.Cryptography.ProtectedData]::Protect($b, $null, 'CurrentUser'))";
    const UNSEAL: &str = "Add-Type -AssemblyName System.Security; \
        $b = [Convert]::FromBase64String([Console]::In.ReadToEnd().Trim()); \
        $x = [Security.Cryptography.ProtectedData]::Unprotect($b, $null, 'CurrentUser'); \
        -join ($x | ForEach-Object { $_.ToString('x2') })";

    static UNSEALED: Mutex<Option<HashMap<Vec<u8>, String>>> = Mutex::new(None);

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn unhex(text: &str) -> Option<Vec<u8>> {
        let text = text.trim();
        text.len().is_multiple_of(2).then_some(())?;
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
            .collect()
    }

    /// PowerShell's answer to `script` with `input` on its stdin.
    fn powershell(script: &str, input: &str) -> Result<String, String> {
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        crate::platform::no_window(&mut command);
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let mut stdin = child.stdin.take().ok_or("no stdin")?;
        stdin
            .write_all(input.as_bytes())
            .map_err(|e| e.to_string())?;
        drop(stdin);
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        match out.status.success() {
            true => Ok(String::from_utf8_lossy(&out.stdout).trim().to_string()),
            false => Err(format!("PowerShell ended with {}", out.status)),
        }
    }

    pub fn seal(_id: &str, key: &str) -> Result<Vec<u8>, Error> {
        let sealed = powershell(SEAL, &hex(key.as_bytes()))
            .map_err(|e| Error::Other(format!("The key wasn't sealed: {e}")))?;
        Ok(sealed.into_bytes())
    }

    pub fn unseal(_id: &str, sealed: &[u8]) -> Result<String, Error> {
        let mut unsealed = UNSEALED.lock().unwrap_or_else(|e| e.into_inner());
        let known = unsealed.get_or_insert_with(HashMap::new);
        if let Some(key) = known.get(sealed) {
            return Ok(key.clone());
        }
        let text = powershell(UNSEAL, &String::from_utf8_lossy(sealed))
            .map_err(|e| Error::Other(format!("The key wasn't unsealed: {e}")))?;
        let key = unhex(&text)
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or_else(|| Error::Other("The key wasn't unsealed".into()))?;
        known.insert(sealed.to_vec(), key.clone());
        Ok(key)
    }

    pub fn forget(_id: &str) {}
}
/// Not compiled here (Windows only): CI builds it on a Mac.
#[cfg(target_os = "macos")]
mod imp {
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };

    use crate::Error;

    const SERVICE: &str = "Gathr tool key";
    /// What the database holds for a key kept in the Keychain.
    const MARKER: &[u8] = b"keychain";

    pub fn seal(id: &str, key: &str) -> Result<Vec<u8>, Error> {
        set_generic_password(SERVICE, id, key.as_bytes())
            .map_err(|e| Error::Other(format!("The key wasn't kept in the Keychain: {e}")))?;
        Ok(MARKER.to_vec())
    }

    pub fn unseal(id: &str, _sealed: &[u8]) -> Result<String, Error> {
        let key = get_generic_password(SERVICE, id)
            .map_err(|e| Error::Other(format!("The key wasn't read from the Keychain: {e}")))?;
        String::from_utf8(key).map_err(|_| Error::Other("The key isn't text".into()))
    }

    pub fn forget(id: &str) {
        let _ = delete_generic_password(SERVICE, id);
    }
}

/// Neither Windows nor a Mac: Gathr ships for no other system, so a key isn't kept there at all.
#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    use crate::Error;

    fn refused() -> Error {
        Error::Refused("Keys can't be kept on this system yet".into())
    }

    pub fn seal(_id: &str, _key: &str) -> Result<Vec<u8>, Error> {
        Err(refused())
    }

    pub fn unseal(_id: &str, _sealed: &[u8]) -> Result<String, Error> {
        Err(refused())
    }

    pub fn forget(_id: &str) {}
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn a_sealed_key_comes_back_and_is_not_stored_as_its_text() {
        let key = "made-up-key-Mira-4417";
        let sealed = super::seal("t1", key).unwrap();
        let found = sealed.windows(key.len()).any(|w| w == key.as_bytes());
        assert!(!found, "the key's bytes are in what's stored");
        assert_eq!(super::unseal("t1", &sealed).unwrap(), key);
    }
}
