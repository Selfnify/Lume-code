'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const root = path.resolve(__dirname, '../..');
const pkg = require('../package.json');
const targets = {
  'win32-x64': ['lume-code.exe', 'ast-grep.exe'],
  'darwin-arm64': ['lume-code', 'ast-grep'],
  'darwin-x64': ['lume-code', 'ast-grep']
};
const key = `${process.platform}-${process.arch}`;
const manifest = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
const version = manifest.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (version !== pkg.version) throw new Error('Cargo.toml and npm/package.json versions must match.');
function cargo(cwd, args, env = process.env) {
  const result = spawnSync('cargo', args, { cwd, stdio: 'inherit', shell: false, env });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status || 1);
}
const externalBinaries = process.env.LUME_SKIP_NATIVE_BUILD === '1';
if (externalBinaries) {
  for (const [key, [executable, astra]] of Object.entries(targets)) {
    const folder = path.join(__dirname, '..', 'native', key);
    for (const name of [executable, astra]) {
      const file = path.join(folder, name);
      if (!fs.existsSync(file)) throw new Error(`Missing release binary ${key}/${name}.`);
    }
  }
  console.log('Verified all staged Windows and macOS releases.');
  process.exit(0);
}
const names = targets[key];
if (!names) {
  throw new Error('Build this release on Windows x64, macOS arm64 or macOS x64.');
}
const [executableName, astraName] = names;
// Build every bundled native executable as part of packing so local installs,
// npm pack, and npm publish all produce a self-contained package.
cargo(root, ['build', '--release', '--quiet', '--locked']);
cargo(path.join(root, 'third-party', 'ast-grep'), ['build', '--release', '--quiet', '--locked', '-p', 'ast-grep']);
const native = path.join(__dirname, '..', 'native', key);
fs.mkdirSync(native, { recursive: true });
for (const oldName of ['lume-code.exe', 'ast-grep.exe', 'lume-code', 'ast-grep']) {
  fs.rmSync(path.join(__dirname, '..', 'native', oldName), { force: true });
}
const executable = path.join(native, executableName);
fs.copyFileSync(path.join(root, 'target', 'release', executableName), executable);
const astra = path.join(root, 'third-party', 'ast-grep', 'target', 'release', astraName);
if (!fs.existsSync(astra)) throw new Error('ASTRA binary is missing after its build completed.');
const astraTarget = path.join(native, astraName);
fs.copyFileSync(astra, astraTarget);
// The Rust app searches beside its own executable for ASTRA.
// Keep the package directory layout aligned with that lookup.
if (process.platform !== 'win32') {
  fs.chmodSync(executable, 0o755);
  fs.chmodSync(astraTarget, 0o755);
}
cargo(root, ['test', '--quiet', '--locked'], { ...process.env, LUME_ASTRA_BIN: astraTarget });
console.log(`Packaged the tested ${key} release.`);
