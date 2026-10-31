'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { run } = require('../bin/lume.cjs');

test('forwards arguments literally and preserves the caller working directory and native exit status', () => {
  const args = ['--project', 'C:\\a folder\\project & more', 'login'];
  let called = false;
  const status = run(args, {
    platform: 'win32', arch: 'x64', executable: process.execPath,
    spawn(exe, received, options) {
      called = true;
      assert.equal(exe, process.execPath);
      assert.deepEqual(received, args);
      assert.equal(options.shell, false);
        assert.equal(options.env.LUME_ASTRA_BIN.endsWith('ast-grep.exe'), true);
      assert.equal(options.stdio, 'inherit');
      assert.equal(options.cwd, undefined);
      return { status: 23 };
    }
  });
  assert.ok(called);
  assert.equal(status, 23);
});
test('help and version do not start the application or require sign-in', () => {
  for (const argument of ['--help', '--version']) {
    let output = '';
    assert.equal(run([argument], { stdout: { write: text => { output += text; } } }), 0);
    assert.match(output, /Lume Code/);
  }
});
test('unsupported platforms, missing binaries and launch failures report actionable errors', () => {
  let output = '';
  const write = text => { output += text; };
  assert.equal(run([], { platform: 'linux', write }), 1);
  assert.equal(run([], { platform: 'win32', arch: 'x64', executable: __filename + '.missing', write }), 1);
  assert.equal(run([], { platform: 'win32', arch: 'x64', executable: process.execPath, write,
    spawn: () => ({ error: new Error('denied') }) }), 1);
  assert.match(output, /Windows x64/);
  assert.match(output, /macOS/);
  assert.match(output, /Reinstall/);
  assert.match(output, /denied/);
});
test('macOS builds start through the same launcher on both architectures', () => {
  for (const arch of ['arm64', 'x64']) {
    let called = false;
    const status = run(['login'], {
      platform: 'darwin', arch, executable: process.execPath,
      spawn(exe, received, options) {
        called = true;
        assert.equal(exe, process.execPath);
        assert.deepEqual(received, ['login']);
        assert.equal(options.shell, false);
        assert.ok(options.env.LUME_ASTRA_BIN.endsWith('ast-grep'));
        return { status: 0 };
      }
    });
    assert.ok(called);
    assert.equal(status, 0);
  }
});
