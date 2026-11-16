'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..');
const pkg = require('../package.json');

test('the manifest carries what the MCP registry verifies', () => {
  assert.equal(pkg.name, 'zpolacode-mcp');
  assert.equal(pkg.mcpName, 'io.github.talhayksk/zpolacode');
  assert.equal(pkg.bin['zpolacode-mcp'], 'bin/zpolacode-mcp.js');
  assert.equal(pkg.repository.url, 'https://github.com/talhayksk/zedpolacode');
  assert.equal(pkg.engines.node, '>=18');
});

test('the description fits the registry limit of 100 characters', () => {
  assert.equal(typeof pkg.description, 'string');
  assert.ok(pkg.description.length <= 100, `description is ${pkg.description.length} characters`);
});

test('the version is a plain semver release version', () => {
  assert.match(pkg.version, /^\d+\.\d+\.\d+$/);
});

test('the tarball contains every file the entry points require', () => {
  // `files` filters the tarball: a directory missing here is missing at
  // install time, and postinstall would fail with ENOENT.
  for (const entry of ['bin', 'lib', 'scripts']) {
    assert.ok(pkg.files.includes(entry), `${entry} must be published`);
  }
  assert.equal(pkg.scripts.postinstall, 'node scripts/install.js');
  assert.ok(fs.existsSync(path.join(ROOT, 'scripts', 'install.js')));
  assert.ok(fs.existsSync(path.join(ROOT, 'bin', 'zpolacode-mcp.js')));
  assert.ok(fs.existsSync(path.join(ROOT, 'lib', 'install.js')));
  assert.ok(fs.existsSync(path.join(ROOT, 'LICENSE')));
  assert.ok(fs.existsSync(path.join(ROOT, 'README.md')));
});

test('the package is MIT, matching the repository license', () => {
  assert.equal(pkg.license, 'MIT');
  assert.match(fs.readFileSync(path.join(ROOT, 'LICENSE'), 'utf8'), /^MIT License\r?\n\r?\nCopyright \(c\) 2026/);
});
