'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const { runPostinstall } = require('../lib/install');

const SCRIPT = path.join(__dirname, '..', 'scripts', 'install.js');

function temporaryDirectory() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'polacode-postinstall-test-'));
}

/** Runs the real postinstall script with a controlled environment. */
function runScript(overrides) {
  return spawnSync(process.execPath, [SCRIPT], {
    encoding: 'utf8',
    env: { ...process.env, POLACODE_MCP_BINARY: '', POLACODE_MCP_CACHE_DIR: '', ...overrides },
  });
}

test('a failing download is reported once and the step still succeeds', async () => {
  const lines = [];
  const code = await runPostinstall({
    env: { POLACODE_MCP_CACHE_DIR: temporaryDirectory() },
    download: async () => {
      throw new Error('getaddrinfo ENOTFOUND github.com');
    },
    log: (line) => lines.push(line),
  });

  assert.equal(code, 0);
  assert.equal(lines.length, 1);
  assert.match(lines[0], /getaddrinfo ENOTFOUND github\.com/);
  assert.match(lines[0], /will be downloaded on the first run/);
});

test('the script exits 0 when the cache cannot even be created', () => {
  const root = temporaryDirectory();
  const blocker = path.join(root, 'cache-root-is-a-file');
  fs.writeFileSync(blocker, 'this is a file, so it cannot hold a cache directory');

  // No network is reached: the directory is created before the request.
  const result = runScript({ POLACODE_MCP_CACHE_DIR: blocker });

  assert.equal(result.status, 0, result.stderr);
  const lines = result.stdout.trim().split(/\r?\n/);
  assert.equal(lines.length, 1, result.stdout);
  assert.match(lines[0], /it will be downloaded on the first run/);
});

test('the script reports an already pinned binary', () => {
  const pinned = path.join(temporaryDirectory(), 'polacode-mcp.exe');
  fs.writeFileSync(pinned, 'stand-in binary');

  const result = runScript({ POLACODE_MCP_BINARY: pinned });

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /binary ready at/);
  assert.match(result.stdout, /polacode-mcp\.exe/);
});
