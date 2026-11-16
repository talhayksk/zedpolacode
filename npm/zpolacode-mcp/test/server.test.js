'use strict';

// server.json is what the official MCP registry reads. It duplicates facts that
// package.json already owns, so the only way it stays publishable is by being
// checked against package.json on every run.

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..');
const pkg = require('../package.json');
const server = require('../server.json');

test('server.json declares the identity package.json advertises through mcpName', () => {
  assert.equal(server.name, pkg.mcpName);
  assert.equal(server.version, pkg.version);
});

test('the single package entry points at this npm package, at this version', () => {
  assert.ok(Array.isArray(server.packages), 'packages must be an array');
  assert.equal(server.packages.length, 1, 'the registry entry describes one package');

  const [entry] = server.packages;
  assert.equal(entry.registryType, 'npm');
  assert.equal(entry.identifier, pkg.name);
  assert.equal(entry.version, pkg.version);
  assert.equal(entry.transport.type, 'stdio');
});

test('the metadata the registry displays is complete', () => {
  assert.equal(server.title, 'Zed Polacode');
  assert.equal(server.repository.url, pkg.repository.url);
  assert.equal(server.repository.source, 'github');
  assert.ok(server.description.length > 0);
  assert.ok(
    server.description.length <= 100,
    `description is ${server.description.length} characters`
  );
});

test('the schema URL is pinned, not just any server.schema.json', () => {
  assert.equal(
    server.$schema,
    'https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json'
  );
});

test('the manifest lives beside the package it publishes', () => {
  // mcp-publisher validates ownership by fetching the npm package named here,
  // so a stale file elsewhere in the repo would be published from nowhere.
  assert.ok(fs.existsSync(path.join(ROOT, 'server.json')));
  assert.equal(pkg.mcpName.split('/').pop(), 'zpolacode');
});
