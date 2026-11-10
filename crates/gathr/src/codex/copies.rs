//! npm's per-platform Codex archive. Storage and switching are shared with Claude (0025, 0033).
use crate::claude::copies::{Copies, Current, Updated};
use crate::data::DataDir;
use crate::fetch::{self, Failure, Redirects, failed};
use serde_json::Value;
use sha2::{Digest, Sha512};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub const FEED_ENV: &str = "GATHR_CODEX_RELEASES";
const REGISTRY: &str = "https://registry.npmjs.org/@openai/codex";
const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;

pub fn feed() -> Option<String> {
    std::env::var(FEED_ENV)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .or_else(|| (!cfg!(debug_assertions)).then(|| REGISTRY.to_string()))
}

fn target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", _) => "x86_64-pc-windows-msvc",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", _) => "x86_64-apple-darwin",
        _ => "unsupported",
    }
}

pub fn storage(data: &DataDir) -> Copies {
    let binary = if cfg!(windows) { "codex.exe" } else { "codex" };
    let relative = PathBuf::from("vendor")
        .join(target())
        .join("bin")
        .join(binary);
    Copies::for_engine(data, "codex", relative)
}

fn json(url: &str, stop: &AtomicBool) -> Result<Value, String> {
    let text = fetch::fetch(url, Redirects::Refused, stop)?;
    serde_json::from_str(&text).map_err(|e| format!("npm metadata isn't JSON: {e}"))
}

pub fn update(data: &DataDir, feed: &str, stop: &AtomicBool) -> Result<Updated, Failure> {
    let storage = storage(data);
    let platform = crate::platform::release_target()
        .ok_or_else(|| failed("platform")("Codex has no build for this computer".into()))?;
    let latest = json(&format!("{feed}/latest"), stop).map_err(failed("feed"))?;
    let version = latest["version"]
        .as_str()
        .filter(|v| fetch::parse_version(v).is_some())
        .ok_or_else(|| failed("manifest")("npm returned no x.y.z version".into()))?;
    let current = storage
        .current()
        .filter(|c| storage.path(&c.version).is_file());
    if let Some(current) = &current
        && fetch::parse_version(&current.version) >= fetch::parse_version(version)
    {
        return Ok(Updated::UpToDate(current.version.clone()));
    }
    let package = format!("{version}-{platform}");
    let manifest = json(&format!("{feed}/{package}"), stop).map_err(failed("manifest"))?;
    if manifest["version"] != package {
        return Err(failed("manifest")(
            "npm returned another platform/version".into(),
        ));
    }
    let url = manifest["dist"]["tarball"].as_str().unwrap_or_default();
    let integrity = manifest["dist"]["integrity"].as_str().unwrap_or_default();
    if !url.starts_with("https://")
        && !(cfg!(debug_assertions) && url.starts_with("http://127.0.0.1:"))
    {
        return Err(failed("manifest")(
            "npm returned no HTTPS tarball URL".into(),
        ));
    }
    if !integrity.starts_with("sha512-") || integrity.len() != 95 {
        return Err(failed("manifest")(
            "npm returned no SHA-512 integrity".into(),
        ));
    }
    install(
        &storage,
        version,
        url,
        integrity,
        current.map(|c| c.version),
        stop,
    )
}

fn install(
    storage: &Copies,
    version: &str,
    url: &str,
    integrity: &str,
    previous: Option<String>,
    stop: &AtomicBool,
) -> Result<Updated, Failure> {
    let disk = |e: std::io::Error| failed("disk")(e.to_string());
    let staging = storage
        .root()
        .join(format!(".download-{}", crate::ids::span_id()));
    let unpacked = staging.join("unpacked");
    std::fs::create_dir_all(&unpacked).map_err(disk)?;
    let archive = staging.join("codex.tgz");
    let folder = storage.root().join(version);
    let aside = staging.join("previous");
    let mut placed = false;
    let result = (|| {
        fetch::transfer(url, &archive, MAX_ARCHIVE, Redirects::Refused, stop)
            .map_err(failed("download"))?;
        let got = integrity_of(&archive).map_err(disk)?;
        if got != integrity {
            return Err(failed("checksum")(format!(
                "Codex archive's SHA-512 is {got}; npm says {integrity}"
            )));
        }
        let sha256 = fetch::sha256_file(&archive).map_err(disk)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(disk)?;
        runtime.block_on(async {
            let archive = archive.display().to_string();
            let into = unpacked.display().to_string();
            let entry = format!("package/vendor/{}", target());
            let env = std::env::vars_os().collect();
            let tar = crate::run::run(
                &crate::platform::tar(),
                &[
                    "-xzf",
                    &archive,
                    "-C",
                    &into,
                    "--strip-components=1",
                    &entry,
                ],
                &staging,
                &env,
                Duration::from_secs(60),
            )
            .await
            .map_err(failed("unpack"))?;
            if tar.code != Some(0) {
                return Err(failed("unpack")(tar.reason().to_string()));
            }
            if folder.exists() {
                std::fs::rename(&folder, &aside).map_err(disk)?;
            }
            crate::platform::move_folder(&unpacked, &folder).map_err(disk)?;
            placed = true;
            let scratch = staging.join("probe");
            let mut client = super::client::Client::start(&storage.path(version), &scratch)
                .await
                .map_err(failed("probe"))?;
            let said = client.version.clone();
            client.engine.close_input();
            // Signed out, no login operation: a stuck probe is safe to end.
            client.engine.close(Duration::from_secs(10)).await;
            if said != version {
                return Err(failed("probe")(format!(
                    "Codex says it's {said}, not {version}"
                )));
            }
            Ok(())
        })?;
        storage.activate(Current {
            version: version.to_string(),
            sha256,
            previous: previous.clone(),
        })
    })();
    if result.is_err() {
        if placed {
            let _ = std::fs::remove_dir_all(&folder);
        }
        if aside.exists() {
            let _ = std::fs::rename(&aside, &folder);
        }
    }
    let _ = std::fs::remove_dir_all(&staging);
    result?;
    Ok(Updated::Installed {
        version: version.to_string(),
        from: previous,
    })
}

fn integrity_of(file: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(file)?;
    let mut sha = Sha512::new();
    let mut bytes = vec![0; 1 << 20];
    loop {
        let n = std::io::Read::read(&mut file, &mut bytes)?;
        if n == 0 {
            break;
        }
        sha.update(&bytes[..n]);
    }
    // The npm integrity field is the standard padded base64 of the 64-byte SHA-512.
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::from("sha512-");
    for chunk in sha.finalize().chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            encoded.push(if i > chunk.len() {
                '='
            } else {
                ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char
            });
        }
    }
    Ok(encoded)
}
