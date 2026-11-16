'use strict';

const { spawn } = require('node:child_process');

// A freshly extracted executable can be refused by Windows security software
// while it is being scanned: CreateProcess fails with ERROR_ACCESS_DENIED,
// which Node surfaces as EPERM. Measured on the machine this package was built
// on: every refusal happened within ~2 minutes of the download and the same
// file started normally afterwards. The retries cover short refusals; they are
// a mitigation, not a guarantee.
const RETRY_CODES = new Set(['EPERM']);
const RETRY_DELAYS_MS = [300, 1200];

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * One actionable line for a failed start: the path, the reason, and — for the
 * transient Windows refusal — what to do about it.
 */
function describeFailure(binary, error) {
  const base = `could not start ${binary}: ${error.message}`;
  if (RETRY_CODES.has(error.code)) {
    return (
      `${base}. Windows refused to run it; security software may still be ` +
      'scanning the freshly downloaded file. Run the command again in a moment.'
    );
  }
  return base;
}

/**
 * Starts `command` with inherited stdio.
 *
 * Resolves with `{ child }` or `{ error }`; it never throws, so the caller can
 * report one line instead of a stack trace. A retryable synchronous failure is
 * retried after each of `delays` ms.
 *
 * `spawn` and `wait` are seams for the tests.
 */
async function spawnBinary(command, args, options = {}) {
  const spawner = options.spawn ?? spawn;
  const wait = options.wait ?? delay;
  const delays = options.delays ?? RETRY_DELAYS_MS;

  for (let attempt = 0; ; attempt += 1) {
    try {
      return { child: spawner(command, args, { stdio: 'inherit' }) };
    } catch (error) {
      if (!RETRY_CODES.has(error.code) || attempt >= delays.length) {
        return { error };
      }
      await wait(delays[attempt]);
    }
  }
}

module.exports = { spawnBinary, describeFailure, RETRY_CODES, RETRY_DELAYS_MS };
