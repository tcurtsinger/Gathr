// Exercise the real release commands against fixture build outputs, without running an installer.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');
const dir = mkdtempSync(join(tmpdir(), 'gathr-release-check-'));
const windows = process.platform === 'win32';
const exe = windows ? '.exe' : '';
const triple = windows ? 'x86_64-pc-windows-msvc' : 'aarch64-apple-darwin';
const targetName = windows ? 'win32-x64' : 'darwin-arm64';
const version = '0.1.999';
const target = join(dir, 'target');
const fixture = join(dir, 'repo');
const out = join(dir, 'out');
const tar = windows ? join(process.env.SystemRoot, 'System32/tar.exe') : '/usr/bin/tar';
function put(path, text) {
  mkdirSync(join(path, '..'), { recursive: true });
  writeFileSync(path, text);
}
try {
  mkdirSync(join(fixture, 'scripts'), { recursive: true });
  copyFileSync(join(root, 'scripts/release.mjs'), join(fixture, 'scripts/release.mjs'));
  for (const program of ['gathrd', 'gathr']) put(join(target, `release/${program}${exe}`), program);
  put(join(fixture, 'ui/dist/index.html'), '<p>Fixture UI</p>');
  const command = (...args) => execFileSync(process.execPath, [join(fixture, 'scripts/release.mjs'), ...args], {
    env: { ...process.env, CARGO_TARGET_DIR: target }, stdio: 'pipe',
  });
  command('sidecar');
  assert.equal(readFileSync(join(fixture, `crates/gathr-app/binaries/gathr-${triple}${exe}`), 'utf8'), 'gathr',
    'sidecar must stage the CLI for the installer');
  const bundle = join(target, 'release/bundle');
  const update = windows ? `nsis/Gathr_${version}_x64-setup.exe` : 'macos/Gathr.app.tar.gz';
  put(join(bundle, update), 'fixture update');
  put(join(bundle, `${update}.sig`), 'fixture signature');
  if (!windows) {
    put(join(bundle, `dmg/Gathr_${version}_aarch64.dmg`), 'fixture dmg');
    for (const program of ['gathrd', 'gathr']) put(join(bundle, `macos/Gathr.app/Contents/MacOS/${program}`), `signed ${program}`);
  }
  command('collect', version, out);
  const unpacked = join(dir, 'unpacked');
  mkdirSync(unpacked);
  execFileSync(tar, ['-xzf', join(out, `gathrd-${version}-${targetName}.tar.gz`), '-C', unpacked]);
  assert.equal(readFileSync(join(unpacked, `gathr${exe}`), 'utf8'), windows ? 'gathr' : 'signed gathr',
    'service updates must carry the same CLI bytes as the app');
  assert.equal(readFileSync(join(unpacked, 'ui/index.html'), 'utf8'), '<p>Fixture UI</p>');
  const config = JSON.parse(readFileSync(join(root, 'crates/gathr-app/tauri.release.conf.json'), 'utf8'));
  assert.ok(config.bundle.externalBin.includes('binaries/gathr'), 'Tauri must package the CLI');
  console.log('Release CLI staging and update archive passed');
} finally {
  rmSync(dir, { recursive: true, force: true });
}
