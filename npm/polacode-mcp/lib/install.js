'use strict';

// Download side of the npm wrapper: fetch the release zip for this platform,
// unpack it into a version-scoped cache, and hand the binary path back.
//
// The binary is the product; this file only moves bytes. Every step is
// injectable so the tests never touch the network.

const fs = require('node:fs');
const https = require('node:https');
const os = require('node:os');
const path = require('node:path');

const { binaryName, tripleFor } = require('./platform');
const { extractToFile } = require('./zip');

const { version: PACKAGE_VERSION } = require('../package.json');

const RELEASE_BASE = 'https://github.com/talhayksk/zedpolacode/releases/download';
const MAX_REDIRECTS = 5;
const REQUEST_TIMEOUT_MS = 30_000;
const USER_AGENT = 'polacode-mcp-npm';

/** The contracted asset URL: one zip per release version and target triple. */
function assetUrl(version, triple) {
  return `${RELEASE_BASE}/mcp-v${version}/polacode-mcp-${triple}.zip`;
}

/** `<homedir>/.polacode-mcp`, or `POLACODE_MCP_CACHE_DIR` when set. */
function cacheRoot(env = process.env) {
  const override = env.POLACODE_MCP_CACHE_DIR;
  return override ? path.resolve(override) : path.join(os.homedir(), '.polacode-mcp');
}

/** `<root>/<version>/<triple>`: a new release or platform never reuses a file. */
function cacheDir({ version, triple, env = process.env }) {
  return path.join(cacheRoot(env), version, triple);
}

/** GETs `url`, follows redirects, resolves with the body. */
function download(url, redirectsLeft = MAX_REDIRECTS) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, { headers: { 'user-agent': USER_AGENT } }, (response) => {
      const status = response.statusCode ?? 0;
      const location = response.headers.location;

      if (status >= 300 && status < 400 && location) {
        response.resume();
        if (redirectsLeft === 0) {
          reject(new Error(`Download failed: more than ${MAX_REDIRECTS} redirects at ${url}.`));
          return;
        }
        resolve(download(new URL(location, url).toString(), redirectsLeft - 1));
        return;
      }
      if (status !== 200) {
        response.resume();
        reject(new Error(`Download failed: HTTP ${status} from ${url}.`));
        return;
      }

      const chunks = [];
      response.on('data', (chunk) => chunks.push(chunk));
      response.on('end', () => resolve(Buffer.concat(chunks)));
      response.on('error', reject);
    });

    request.setTimeout(REQUEST_TIMEOUT_MS, () => {
      request.destroy(new Error(`Download timed out after ${REQUEST_TIMEOUT_MS} ms: ${url}.`));
    });
    request.on('error', reject);
  });
}

/** Downloads this version and triple into `dir` as the platform binary. */
async function fetchAndExtract({ version, triple, dir }) {
  // Create the directory before the request so an unwritable cache fails
  // immediately instead of after a 300 KB download.
  fs.mkdirSync(dir, { recursive: true });
  const archive = await download(assetUrl(version, triple));
  extractToFile(archive, binaryName(), path.join(dir, binaryName()));
}

/** True when `file` is a non-empty file and, on POSIX, executable. */
function isInstalled(file) {
  try {
    const stat = fs.statSync(file);
    if (!stat.isFile() || stat.size === 0) {
      return false;
    }
    if (process.platform !== 'win32') {
      fs.accessSync(file, fs.constants.X_OK);
    }
    return true;
  } catch {
    return false;
  }
}

/**
 * Returns the path of a usable binary, downloading it only when the cache has
 * nothing to run.
 *
 * Options, all for tests or for the escape hatch:
 *   version, triple  override the package version / detected platform
 *   env              the environment to read the overrides from
 *   cacheDir         override the computed cache directory
 *   download         replace the network + unzip step
 */
async function ensureBinary(options = {}) {
  const env = options.env ?? process.env;

  const pinned = env.POLACODE_MCP_BINARY;
  if (pinned) {
    const resolved = path.resolve(pinned);
    if (!fs.existsSync(resolved)) {
      throw new Error(`POLACODE_MCP_BINARY points at ${resolved}, which does not exist.`);
    }
    return resolved;
  }

  const version = options.version ?? PACKAGE_VERSION;
  const triple = options.triple ?? tripleFor();
  const dir = options.cacheDir ?? cacheDir({ version, triple, env });
  const target = path.join(dir, binaryName());

  if (isInstalled(target)) {
    return target;
  }

  const downloader = options.download ?? fetchAndExtract;
  await downloader({ version, triple, dir, url: assetUrl(version, triple) });

  if (!isInstalled(target)) {
    throw new Error(`Nothing runnable at ${target} after the download.`);
  }
  return target;
}

/**
 * The postinstall step. It never throws and always reports success: a machine
 * without a network still gets a working package, because `bin/polacode-mcp.js`
 * downloads on the first run.
 */
async function runPostinstall(options = {}) {
  const log = options.log ?? ((line) => process.stdout.write(`${line}\n`));
  try {
    const binary = await ensureBinary(options);
    log(`polacode-mcp: binary ready at ${binary}`);
  } catch (error) {
    log(
      `polacode-mcp: could not fetch the binary now (${error.message}); ` +
        'it will be downloaded on the first run.'
    );
  }
  return 0;
}

module.exports = {
  assetUrl,
  cacheRoot,
  cacheDir,
  download,
  fetchAndExtract,
  isInstalled,
  ensureBinary,
  runPostinstall,
};
