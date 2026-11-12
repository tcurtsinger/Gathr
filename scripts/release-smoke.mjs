// CI only: installing NSIS touches the runner's app registration. Never install over a person's app.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

assert.equal(process.env.GITHUB_ACTIONS, 'true', 'Installer smoke runs only on disposable GitHub runners');
assert.ok(process.env.RUNNER_TEMP, 'RUNNER_TEMP is required');
const [version, output] = process.argv.slice(2);
assert.match(version, /^\d+\.\d+\.\d+$/);
assert.ok(output, 'release output folder is required');
const dir = mkdtempSync(join(process.env.RUNNER_TEMP, 'gathr-release-smoke-'));
const windows = process.platform === 'win32';
const exe = windows ? '.exe' : '';
const target = process.env.CARGO_TARGET_DIR ?? join(import.meta.dirname, '../target');
const installed = windows ? join(dir, 'installed') : join(target, 'release/bundle/macos/Gathr.app/Contents/MacOS');
const report = [];
const run = (program, args) => execFileSync(program, args, { encoding: 'utf8', timeout: 120_000, windowsHide: true });
function check(folder, label) {
  for (const name of ['gathr', 'gathrd']) {
    // Both Clap commands report their crate name, `gathr`, including the service executable.
    assert.equal(run(join(folder, name + exe), ['--version']).trim(), `gathr ${version}`, `${label}: ${name} version`);
  }
  const data = join(dir, label + '-data');
  run(join(folder, 'gathr' + exe), ['seed', '--data', data]);
  const health = JSON.parse(run(join(folder, 'gathr' + exe), ['diag', 'health', '--data', data]));
  assert.equal(health.database.opened, true, `${label}: seeded database opens`);
  assert.equal(resolve(health.database.path), resolve(join(data, 'gathr.db')), `${label}: explicit test data used`);
  report.push({ location: label, version, database_opened: health.database.opened, explicit_test_data: true });
}
try {
  if (windows) run(resolve(output, `Gathr_${version}_x64-setup.exe`), ['/S', `/D=${installed}`]);
  check(installed, windows ? 'nsis-install' : 'mac-app');
  const unpacked = join(dir, 'service-update');
  mkdirSync(unpacked);
  const platform = windows ? 'win32-x64' : 'darwin-arm64';
  const tar = windows ? join(process.env.SystemRoot, 'System32/tar.exe') : '/usr/bin/tar';
  run(tar, ['-xzf', resolve(output, `gathrd-${version}-${platform}.tar.gz`), '-C', unpacked]);
  check(unpacked, 'service-update');
  writeFileSync(join(output, `diag-smoke-${platform}.json`), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report));
} finally {
  // The runner is disposable: do not start the window, engines or an uninstaller's copied process.
  rmSync(dir, { recursive: true, force: true });
}
