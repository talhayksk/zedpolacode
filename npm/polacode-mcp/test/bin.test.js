'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const BIN = path.join(__dirname, '..', 'bin', 'polacode-mcp.js');

function temporaryDirectory() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'polacode-bin-test-'));
}

/**
 * Writes a stand-in binary as a Node script. The wrapper runs a `.js`
 * POLACODE_MCP_BINARY through its own Node, which is the documented escape
 * hatch; the released binary is a native executable and takes the plain path.
 */
function writeFakeBinary(body) {
  const file = path.join(temporaryDirectory(), 'fake-binary.js');
  fs.writeFileSync(file, `'use strict';\n${body}\n`);
  return file;
}

function runBin(args, { env = {}, input } = {}) {
  return spawnSync(process.execPath, [BIN, ...args], {
    encoding: 'utf8',
    input,
    env: {
      ...process.env,
      POLACODE_MCP_CACHE_DIR: temporaryDirectory(),
      POLACODE_MCP_BINARY: '',
      ...env,
    },
  });
}

test('arguments reach the binary unchanged', () => {
  const fake = writeFakeBinary('process.stdout.write(JSON.stringify(process.argv.slice(2)));');

  const result = runBin(
    ['card', '--clipboard', '--lang', 'Rust', '--title', 'npm e2e.rs', '--open'],
    { env: { POLACODE_MCP_BINARY: fake } }
  );

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), [
    'card',
    '--clipboard',
    '--lang',
    'Rust',
    '--title',
    'npm e2e.rs',
    '--open',
  ]);
});

test('stdin and stdout are inherited, so the MCP stdio protocol flows through', () => {
  const fake = writeFakeBinary(
    [
      'let input = "";',
      'process.stdin.setEncoding("utf8");',
      'process.stdin.on("data", (chunk) => { input += chunk; });',
      'process.stdin.on("end", () => { process.stdout.write(input.toUpperCase()); });',
    ].join('\n')
  );

  const result = runBin([], {
    env: { POLACODE_MCP_BINARY: fake },
    input: '{"jsonrpc":"2.0","id":1,"method":"ping"}\n',
  });

  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, '{"JSONRPC":"2.0","ID":1,"METHOD":"PING"}\n');
});

test('the exit code of the binary becomes the exit code of the wrapper', () => {
  const seven = writeFakeBinary('process.exit(7);');
  const zero = writeFakeBinary('process.exit(0);');

  assert.equal(runBin([], { env: { POLACODE_MCP_BINARY: seven } }).status, 7);
  assert.equal(runBin([], { env: { POLACODE_MCP_BINARY: zero } }).status, 0);
});

test('a missing pinned binary is reported, not thrown as a stack trace', () => {
  const missing = path.join(temporaryDirectory(), 'not-here.exe');

  const result = runBin([], { env: { POLACODE_MCP_BINARY: missing } });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /^polacode-mcp: POLACODE_MCP_BINARY points at /);
  assert.equal(result.stdout, '');
});

test('a target that cannot be executed is reported, not thrown as a stack trace', () => {
  const notABinary = path.join(temporaryDirectory(), 'not-a-binary.txt');
  fs.writeFileSync(notABinary, 'this is not an executable');

  const result = runBin([], { env: { POLACODE_MCP_BINARY: notABinary } });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /^polacode-mcp: could not start .*not-a-binary\.txt: /);
  assert.doesNotMatch(result.stderr, /at main \(/);
});

test('the wrapper starts with a shebang so npx can run it directly', () => {
  assert.ok(fs.readFileSync(BIN, 'utf8').startsWith('#!/usr/bin/env node\n'));
});
