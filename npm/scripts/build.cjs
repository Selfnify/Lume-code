'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const root = path.resolve(__dirname, '../..');
const pkg = require('../package.json');
if (process.platform !== 'win32' || process.arch !== 'x64') {
  throw new Error('Build this release on Windows x64.');
}
const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
const version = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (version !== pkg.version) throw new Error('Cargo.toml and npm/package.json versions must match.');
for (const args of [['test', '--quiet'], ['build', '--release', '--quiet', '--locked']]) {
  const result = spawnSync('cargo', args, { cwd: root, stdio: 'inherit', shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status || 1);
}
const native = path.join(__dirname, '..', 'native');
fs.mkdirSync(native, { recursive: true });
fs.copyFileSync(path.join(root, 'target/release/lume-code.exe'), path.join(native, 'lume-code.exe'));
const astra = path.join(root, 'third-party', 'ast-grep', 'target', 'release', 'ast-grep.exe');
if (!fs.existsSync(astra)) throw new Error('ASTRA binary is missing; build third-party/ast-grep first.');
fs.copyFileSync(astra, path.join(native, 'ast-grep.exe'));
console.log('Packaged the tested Windows x64 release.');
