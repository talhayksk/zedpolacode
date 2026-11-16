#!/usr/bin/env node
'use strict';

// The npm entry point: make sure the platform binary is on disk, then become
// it. With no arguments the binary is the MCP stdio server, so
// `npx -y zpolacode-mcp` is a complete server command for any MCP client.

const path = require('node:path');

const { ensureBinary } = require('../lib/install');
const { spawnBinary, describeFailure } = require('../lib/spawn-binary');

const SIGNALS = ['SIGINT', 'SIGTERM', 'SIGHUP'];

// A `.js` target is only reachable through ZPOLACODE_MCP_BINARY (the released
// binary is an ELF/Mach-O/PE file). Running it with this same Node keeps a
// script shim usable on every platform.
const NODE_SCRIPT = new Set(['.js', '.cjs', '.mjs']);

function reportSpawnFailure(binary, error) {
  process.stderr.write(`zpolacode-mcp: ${describeFailure(binary, error)}\n`);
  process.exitCode = 1;
}

async function main() {
  const args = process.argv.slice(2);

  let binary;
  try {
    binary = await ensureBinary();
  } catch (error) {
    process.stderr.write(`zpolacode-mcp: ${error.message}\n`);
    process.exitCode = 1;
    return;
  }

  const runScript = NODE_SCRIPT.has(path.extname(binary).toLowerCase());

  const { child, error } = runScript
    ? await spawnBinary(process.execPath, [binary, ...args])
    : await spawnBinary(binary, args);

  if (error) {
    // A stack trace would be noise in an MCP client's log; the reason and the
    // path are what a user can act on.
    reportSpawnFailure(binary, error);
    return;
  }

  // The client controls this process; forward its signals to the server it
  // started, and take the handler off before re-raising the child's signal.
  const forward = (signal) => {
    try {
      child.kill(signal);
    } catch {
      // The child is already gone; its exit event decides our status.
    }
  };
  for (const signal of SIGNALS) {
    process.on(signal, forward);
  }

  const detach = () => {
    for (const signal of SIGNALS) {
      process.off(signal, forward);
    }
  };

  child.on('error', (error) => {
    detach();
    reportSpawnFailure(binary, error);
  });

  child.on('exit', (code, signal) => {
    detach();
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exitCode = code ?? 1;
  });
}

main();
