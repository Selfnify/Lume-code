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
const names = targets[key];
if (!names) {
  throw new Error('Build this release on Windows x64, macOS arm64 or macOS x64.');
}
const [executableName, astraName] = names;
const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
const version = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (version !== pkg.version) throw new Error('Cargo.toml and npm/package.json versions must match.');
for (const args of [['test', '--quiet'], ['build', '--release', '--quiet', '--locked']]) {
  const result = spawnSync('cargo', args, { cwd: root, stdio: 'inherit', shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status || 1);
}
const native = path.join(__dirname, '..', 'native', key);
fs.mkdirSync(native, { recursive: true });
const executable = path.join(native, executableName);
fs.copyFileSync(path.join(root, 'target', 'release', executableName), executable);
const astra = [
  process.env.LUME_ASTRA_BIN,
  path.join(root, 'third-party', 'ast-grep', 'target', 'release', astraName),
  path.join(__dirname, '..', 'native', astraName)
].filter(Boolean).find(file => fs.existsSync(file));
if (!astra) throw new Error('ASTRA binary is missing. Build third-party/ast-grep, set LUME_ASTRA_BIN, or keep a previous ast-grep in npm/native.');
const astraTarget = path.join(native, astraName);
fs.copyFileSync(astra, astraTarget);
if (process.platform !== 'win32') {
  fs.chmodSync(executable, 0o755);
  fs.chmodSync(astraTarget, 0o755);
}
console.log(`Packaged the tested ${key} release.`);
