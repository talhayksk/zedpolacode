'use strict';

// The five triples the release workflow publishes, one zip each:
// .github/workflows/release-mcp.yml builds every target in this list.
const TRIPLES = {
  'win32-x64': 'x86_64-pc-windows-msvc',
  'darwin-x64': 'x86_64-apple-darwin',
  'darwin-arm64': 'aarch64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
};

const SUPPORTED = Object.keys(TRIPLES);

/**
 * Maps a platform/arch pair to the release triple.
 *
 * Throws for every pair the release does not publish. There is no fallback:
 * running the wrong architecture's binary is worse than saying so.
 */
function tripleFor(platform = process.platform, arch = process.arch) {
  const triple = TRIPLES[`${platform}-${arch}`];
  if (!triple) {
    throw new Error(
      `polacode-mcp has no build for ${platform}-${arch}. ` +
        `Supported platforms: ${SUPPORTED.join(', ')}.`
    );
  }
  return triple;
}

/** The file name inside the release zip for this platform. */
function binaryName(platform = process.platform) {
  return platform === 'win32' ? 'polacode-mcp.exe' : 'polacode-mcp';
}

module.exports = { TRIPLES, SUPPORTED, tripleFor, binaryName };
