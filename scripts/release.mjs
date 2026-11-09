// Gathr's release files, the same in CI (.github/workflows/release.yml) and in a local check.
//
//   node scripts/release.mjs stamp <version>       the workspace's version, in Cargo.toml and Cargo.lock
//   node scripts/release.mjs sidecar               target/release/gathrd beside the window, for `tauri build`
//   node scripts/release.mjs collect <version> <out>
//       after `tauri build`: this system's installer, its update and signature, and gathrd with the built
//       UI as gathrd-<version>-<target>.tar.gz (0031), into <out>, with <out>/<target>.json naming them
//   node scripts/release.mjs feeds <version> <dir> <base url>
//       every <target>.json in <dir> into gathrd.json (the service's feed) and latest.json (the window's)
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, cpSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..");
const target = process.env.CARGO_TARGET_DIR ?? join(root, "target");
const windows = process.platform === "win32";
const exe = windows ? ".exe" : "";
// Cass's targets (platform::release_target) and the updater's, for the systems Gathr ships on.
const systems = {
  win32: { target: "win32-x64", updater: "windows-x86_64", triple: "x86_64-pc-windows-msvc" },
  darwin: { target: "darwin-arm64", updater: "darwin-aarch64", triple: "aarch64-apple-darwin" },
};
const system = systems[process.platform];

function stamp(version) {
  if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) fail(`version ${version} isn't x.y.z`);
  const edit = (file, pattern, by) => {
    const path = join(root, file);
    const text = readFileSync(path, "utf8");
    const next = text.replace(pattern, by);
    if (next === text) fail(`${file}: nothing to stamp`);
    writeFileSync(path, next);
  };
  edit("Cargo.toml", /(\[workspace\.package\]\r?\nversion = )"[^"]*"/, `$1"${version}"`);
  edit("Cargo.lock", /(name = "gathr(?:-app)?"\r?\nversion = )"[^"]*"/g, `$1"${version}"`);
}

function sidecar() {
  const folder = join(root, "crates/gathr-app/binaries");
  mkdirSync(folder, { recursive: true });
  copyFileSync(join(target, `release/gathrd${exe}`), join(folder, `gathrd-${system.triple}${exe}`));
}

function collect(version, out) {
  mkdirSync(out, { recursive: true });
  const bundle = join(target, "release/bundle");
  const [installer, update] = windows
    ? [`nsis/Gathr_${version}_x64-setup.exe`, `nsis/Gathr_${version}_x64-setup.exe`]
    : [`dmg/Gathr_${version}_aarch64.dmg`, "macos/Gathr.app.tar.gz"];
  const updateName = windows ? `Gathr_${version}_x64-setup.exe` : `Gathr_${version}_aarch64.app.tar.gz`;
  copyFileSync(join(bundle, installer), join(out, installer.split("/")[1]));
  copyFileSync(join(bundle, update), join(out, updateName));
  const signature = readFileSync(join(bundle, `${update}.sig`), "utf8").trim();

  // gathrd and the built UI, as the service unpacks them: both at the archive's top. The same bytes the
  // installer puts in: on macOS the bundler signs gathrd again, and a copy that differs from the installed one
  // reads to the service as a rebuild, which it copies in again and so ends a trial with no rollback.
  const staging = join(out, `.gathrd-${system.target}`);
  rmSync(staging, { recursive: true, force: true });
  mkdirSync(staging);
  const gathrd = windows ? join(target, "release/gathrd.exe") : join(bundle, "macos/Gathr.app/Contents/MacOS/gathrd");
  copyFileSync(gathrd, join(staging, `gathrd${exe}`));
  cpSync(join(root, "ui/dist"), join(staging, "ui"), { recursive: true });
  const archive = `gathrd-${version}-${system.target}.tar.gz`;
  execFileSync(tar(), ["-czf", join(out, archive), "-C", staging, `gathrd${exe}`, "ui"], { stdio: "inherit" });
  rmSync(staging, { recursive: true });

  const part = {
    gathrd: { target: system.target, file: archive, ...hashed(join(out, archive)) },
    window: { target: system.updater, file: updateName, signature },
  };
  writeFileSync(join(out, `${system.target}.json`), `${JSON.stringify(part, null, 2)}\n`);
}

function feeds(version, dir, base) {
  const parts = readdirSync(dir)
    .filter((f) => Object.values(systems).some((s) => f === `${s.target}.json`))
    .map((f) => JSON.parse(readFileSync(join(dir, f), "utf8")));
  if (parts.length === 0) fail(`${dir} has no <target>.json`);
  const gathrd = { version, platforms: {} };
  const latest = { version, pub_date: new Date().toISOString(), platforms: {} };
  for (const { gathrd: g, window: w } of parts) {
    gathrd.platforms[g.target] = { file: g.file, sha256: g.sha256, size: g.size };
    latest.platforms[w.target] = { signature: w.signature, url: `${base.replace(/\/$/, "")}/${w.file}` };
  }
  writeFileSync(join(dir, "gathrd.json"), `${JSON.stringify(gathrd, null, 2)}\n`);
  writeFileSync(join(dir, "latest.json"), `${JSON.stringify(latest, null, 2)}\n`);
}

function hashed(file) {
  return { sha256: createHash("sha256").update(readFileSync(file)).digest("hex"), size: statSync(file).size };
}

// The system's own tar (bsdtar on both), by its full path, as the service unpacks with.
function tar() {
  return windows ? join(process.env.SystemRoot ?? "C:\\Windows", "System32\\tar.exe") : "/usr/bin/tar";
}

function fail(why) {
  console.error(`release: ${why}`);
  process.exit(1);
}

const [command, ...args] = process.argv.slice(2);
if (!system && ["sidecar", "collect"].includes(command)) fail(`${process.platform} ships no release`);
const commands = { stamp, sidecar, collect, feeds };
if (!commands[command]) fail("usage: stamp <version> | sidecar | collect <version> <out> | feeds <version> <dir> <base url>");
commands[command](...args);
