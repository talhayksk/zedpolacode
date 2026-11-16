#!/usr/bin/env node
'use strict';

// postinstall: warm the cache so the first `npx -y polacode-mcp` is instant.
// A failure here must never fail the install: the bin downloads on first run.
require('../lib/install')
  .runPostinstall()
  .then((code) => {
    process.exitCode = code;
  })
  .catch(() => {
    // Even an unexpected failure (a closed stdout, say) exits 0: npm must not
    // see this script as the reason an install failed.
    process.exitCode = 0;
  });
