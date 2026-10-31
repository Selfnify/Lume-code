#!/usr/bin/env node
'use strict';

const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { existsSync } = require('node:fs');
const pkg = require('../package.json');
const targets = {
  'win32-x64': 'lume-code.exe',
  'darwin-arm64': 'lume-code',
  'darwin-x64': 'lume-code'
};

function run(args, options = {}) {
  const platform = options.platform || process.platform;
  const arch = options.arch || process.arch;
  const write = options.write || ((text) => process.stderr.write(text));
  if (args.length === 1 && ['--version', '-v'].includes(args[0])) {
    (options.stdout || process.stdout).write(`Lume Code ${pkg.version}\n`);
    return 0;
  }
  if (args.length === 1 && ['--help', '-h'].includes(args[0])) {
    (options.stdout || process.stdout).write(
      'Lume Code\n\nUsage: lume [login|logout] [--project PATH]\n\n' +
      'Run lume in your project folder to start. Sign in with your Lume account.\n' +
      'Inside Lume, type / for commands or ? for keyboard shortcuts.\n');
    return 0;
  }
  const key = `${platform}-${arch}`;
  const binary = targets[key];
  if (!binary) {
    write('Lume Code currently supports Windows x64 and macOS (Apple Silicon and Intel).\n');
    return 1;
  }
  const executable = options.executable || path.join(__dirname, '..', 'native', key, binary);
  if (!existsSync(executable)) {
    write('The Lume executable is missing. Reinstall the npm package.\n');
    return 1;
  }
  const result = (options.spawn || spawnSync)(executable, args, {
    stdio: 'inherit', shell: false, windowsHide: false
  });
  if (result.error) {
    write(`Could not start Lume: ${result.error.message}\n`);
    return 1;
  }
  return result.status ?? (result.signal === 'SIGINT' ? 130 : 1);
}

if (require.main === module) process.exitCode = run(process.argv.slice(2));
module.exports = { run };
