'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const { spawnBinary, describeFailure, RETRY_DELAYS_MS } = require('../lib/spawn-binary');

function errno(code) {
  const error = new Error(`spawn ${code}`);
  error.code = code;
  return error;
}

test('the normal path spawns once with inherited stdio', async () => {
  const calls = [];
  const child = { pid: 1 };

  const result = await spawnBinary('polacode-mcp', ['card'], {
    spawn: (...args) => {
      calls.push(args);
      return child;
    },
    wait: async () => assert.fail('no wait expected'),
  });

  assert.equal(result.child, child);
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0], ['polacode-mcp', ['card'], { stdio: 'inherit' }]);
});

test('a transient EPERM is retried and the next start is used', async () => {
  let attempts = 0;
  const waits = [];
  const child = { pid: 2 };

  const result = await spawnBinary('polacode-mcp.exe', [], {
    spawn: () => {
      attempts += 1;
      if (attempts === 1) {
        throw errno('EPERM');
      }
      return child;
    },
    wait: async (ms) => waits.push(ms),
  });

  assert.equal(result.child, child);
  assert.equal(attempts, 2);
  assert.deepEqual(waits, [RETRY_DELAYS_MS[0]]);
});

test('a second transient EPERM uses the next delay', async () => {
  let attempts = 0;
  const waits = [];
  const child = { pid: 3 };

  const result = await spawnBinary('polacode-mcp.exe', [], {
    spawn: () => {
      attempts += 1;
      if (attempts <= 2) {
        throw errno('EPERM');
      }
      return child;
    },
    wait: async (ms) => waits.push(ms),
  });

  assert.equal(result.child, child);
  assert.equal(attempts, 3);
  assert.deepEqual(waits, RETRY_DELAYS_MS);
});

test('a persistent EPERM is reported after the last retry, not looped on', async () => {
  let attempts = 0;

  const result = await spawnBinary('polacode-mcp.exe', [], {
    spawn: () => {
      attempts += 1;
      throw errno('EPERM');
    },
    wait: async () => {},
  });

  assert.equal(attempts, RETRY_DELAYS_MS.length + 1);
  assert.equal(result.error.code, 'EPERM');
  assert.equal(result.child, undefined);
});

test('any other failure is reported immediately', async () => {
  let attempts = 0;

  const result = await spawnBinary('polacode-mcp.exe', [], {
    spawn: () => {
      attempts += 1;
      throw errno('ENOENT');
    },
    wait: async () => assert.fail('no retry expected'),
  });

  assert.equal(attempts, 1);
  assert.equal(result.error.code, 'ENOENT');
});

test('a refused start names the path and tells the user to try again', () => {
  const message = describeFailure('C:\\cache\\polacode-mcp.exe', errno('EPERM'));

  assert.match(message, /^could not start C:\\cache\\polacode-mcp\.exe: spawn EPERM/);
  assert.match(message, /Run the command again in a moment/);
});

test('a non-transient failure carries no retry advice', () => {
  const message = describeFailure('/usr/local/bin/polacode-mcp', errno('ENOENT'));

  assert.equal(message, 'could not start /usr/local/bin/polacode-mcp: spawn ENOENT');
});
