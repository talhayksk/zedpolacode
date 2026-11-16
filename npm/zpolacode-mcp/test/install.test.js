'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { assetUrl, cacheDir, cacheRoot, ensureBinary } = require('../lib/install');
const { binaryName, tripleFor } = require('../lib/platform');
const { version } = require('../package.json');

function temporaryDirectory() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'polacode-install-test-'));
}

/** Writes a stand-in binary where the downloader would have put it. */
function fakeDownloader(counter = { calls: 0 }) {
  return async ({ dir }) => {
    counter.calls += 1;
    fs.mkdirSync(dir, { recursive: true });
    const target = path.join(dir, binaryName());
    fs.writeFileSync(target, 'fake binary');
    if (process.platform !== 'win32') {
      fs.chmodSync(target, 0o755);
    }
  };
}

test('the asset URL is the release contract for this package version', () => {
  assert.equal(
    assetUrl(version, 'x86_64-pc-windows-msvc'),
    `https://github.com/talhayksk/zedpolacode/releases/download/mcp-v${version}/zpolacode-mcp-x86_64-pc-windows-msvc.zip`
  );
  assert.match(version, /^\d+\.\d+\.\d+$/);
});

test('the cache lives under the version and the triple', () => {
  const home = path.join(os.homedir(), '.zpolacode-mcp');
  assert.equal(cacheRoot({}), home);
  assert.equal(cacheRoot({ ZPOLACODE_MCP_CACHE_DIR: path.join('some', 'where') }), path.resolve('some', 'where'));
  assert.equal(
    cacheDir({ version: '1.2.3', triple: 'x86_64-unknown-linux-gnu', env: {} }),
    path.join(home, '1.2.3', 'x86_64-unknown-linux-gnu')
  );
});

test('a cached binary is reused and never downloaded twice', async () => {
  const root = temporaryDirectory();
  const env = { ZPOLACODE_MCP_CACHE_DIR: root };
  const counter = { calls: 0 };
  const download = fakeDownloader(counter);

  const first = await ensureBinary({ env, download });
  const second = await ensureBinary({ env, download });

  assert.equal(counter.calls, 1);
  assert.equal(first, second);
  assert.equal(first, path.join(root, version, tripleFor(), binaryName()));
  assert.equal(fs.readFileSync(first, 'utf8'), 'fake binary');
});

test('the downloader is told the version, the triple and the cache directory', async () => {
  const root = temporaryDirectory();
  const seen = [];
  const download = async (request) => {
    seen.push(request);
    await fakeDownloader()(request);
  };

  await ensureBinary({ env: { ZPOLACODE_MCP_CACHE_DIR: root }, download });

  assert.equal(seen.length, 1);
  assert.equal(seen[0].version, version);
  assert.equal(seen[0].triple, tripleFor());
  assert.equal(seen[0].dir, path.join(root, version, tripleFor()));
  assert.equal(seen[0].url, assetUrl(version, tripleFor()));
});

test('ZPOLACODE_MCP_BINARY skips both the cache and the download', async () => {
  const pinned = path.join(temporaryDirectory(), 'shim.js');
  fs.writeFileSync(pinned, '// stand-in');

  let calls = 0;
  const binary = await ensureBinary({
    env: { ZPOLACODE_MCP_BINARY: pinned },
    download: async () => {
      calls += 1;
    },
  });

  assert.equal(binary, path.resolve(pinned));
  assert.equal(calls, 0);
});

test('ZPOLACODE_MCP_BINARY pointing at nothing is an error naming the variable', async () => {
  await assert.rejects(
    ensureBinary({ env: { ZPOLACODE_MCP_BINARY: path.join(temporaryDirectory(), 'gone') }, download: async () => {} }),
    /ZPOLACODE_MCP_BINARY points at .* which does not exist/
  );
});

test('a downloader that writes nothing is caught before the binary is run', async () => {
  await assert.rejects(
    ensureBinary({ env: { ZPOLACODE_MCP_CACHE_DIR: temporaryDirectory() }, download: async () => {} }),
    /Nothing runnable at .* after the download/
  );
});

test('a downloader that fails surfaces its own reason', async () => {
  await assert.rejects(
    ensureBinary({
      env: { ZPOLACODE_MCP_CACHE_DIR: temporaryDirectory() },
      download: async () => {
        throw new Error('Download failed: HTTP 404 from https://example.invalid/x.zip.');
      },
    }),
    /HTTP 404/
  );
});

test('an empty cached file is not treated as installed', async () => {
  const root = temporaryDirectory();
  const dir = path.join(root, version, tripleFor());
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, binaryName()), '');

  const counter = { calls: 0 };
  await ensureBinary({ env: { ZPOLACODE_MCP_CACHE_DIR: root }, download: fakeDownloader(counter) });

  assert.equal(counter.calls, 1);
});
