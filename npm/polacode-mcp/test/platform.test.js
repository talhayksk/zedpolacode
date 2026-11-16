'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const { TRIPLES, tripleFor, binaryName } = require('../lib/platform');

const PUBLISHED = [
  ['win32', 'x64', 'x86_64-pc-windows-msvc'],
  ['darwin', 'x64', 'x86_64-apple-darwin'],
  ['darwin', 'arm64', 'aarch64-apple-darwin'],
  ['linux', 'x64', 'x86_64-unknown-linux-gnu'],
  ['linux', 'arm64', 'aarch64-unknown-linux-gnu'],
];

test('every published pair maps to its release triple', () => {
  for (const [platform, arch, triple] of PUBLISHED) {
    assert.equal(tripleFor(platform, arch), triple, `${platform}-${arch}`);
  }
  assert.deepEqual(
    Object.values(TRIPLES).sort(),
    PUBLISHED.map((entry) => entry[2]).sort()
  );
});

test('an unsupported pair names the pair and the supported list', () => {
  // win32-arm64 has no asset in the release; freebsd is not a target at all.
  for (const [platform, arch] of [['win32', 'arm64'], ['freebsd', 'x64']]) {
    assert.throws(
      () => tripleFor(platform, arch),
      (error) => {
        assert.match(error.message, new RegExp(`${platform}-${arch}`));
        for (const [supportedPlatform, supportedArch] of PUBLISHED) {
          assert.match(error.message, new RegExp(`${supportedPlatform}-${supportedArch}`));
        }
        return true;
      }
    );
  }
});

test('the binary name follows the platform', () => {
  assert.equal(binaryName('win32'), 'polacode-mcp.exe');
  assert.equal(binaryName('linux'), 'polacode-mcp');
  assert.equal(binaryName('darwin'), 'polacode-mcp');
});
