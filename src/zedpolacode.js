// Zed Polacode renderer (runs on the host with Node).
//
// The Zed extension sandbox can only write inside its own directory, so the
// extension hands the finished HTML to this script. It copies the card next to
// the project and, when a Chromium-based browser is available, screenshots it.
//
// Usage (all values come in as one JSON argument):
//   node -e <script> '{"mode":"file","html":"...","outDir":"...","base":"main-L1-20", ...}'
//
// Output is a few `key=value` lines on stdout, parsed by the extension.

'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

function emit(lines) {
  process.stdout.write(lines.filter(Boolean).join('\n') + '\n');
}

function isFile(candidate) {
  try {
    return Boolean(candidate) && fs.statSync(candidate).isFile();
  } catch (error) {
    return false;
  }
}

function fileUrl(target) {
  let resolved = path.resolve(target).replace(/\\/g, '/');
  if (!resolved.startsWith('/')) {
    resolved = '/' + resolved;
  }
  return 'file://' + resolved;
}

function which(name) {
  try {
    const probe = spawnSync(process.platform === 'win32' ? 'where' : 'which', [name], {
      encoding: 'utf8',
      timeout: 5000,
      windowsHide: true,
    });
    if (probe.status !== 0 || !probe.stdout) {
      return null;
    }
    const first = probe.stdout
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)[0];
    return isFile(first) ? first : null;
  } catch (error) {
    return null;
  }
}

function findBrowser() {
  const explicit = [
    process.env.ZEDPOLACODE_BROWSER,
    process.env.POLACODE_BROWSER,
    process.env.CHROME_PATH,
    process.env.PUPPETEER_EXECUTABLE_PATH,
  ];
  for (const candidate of explicit) {
    if (isFile(candidate)) {
      return candidate;
    }
  }

  const candidates = [];
  if (process.platform === 'win32') {
    const programFiles = process.env['ProgramFiles'] || 'C:\\Program Files';
    const programFilesX86 = process.env['ProgramFiles(x86)'] || 'C:\\Program Files (x86)';
    const localAppData = process.env['LOCALAPPDATA'];
    candidates.push(
      path.join(programFiles, 'Google', 'Chrome', 'Application', 'chrome.exe'),
      path.join(programFilesX86, 'Google', 'Chrome', 'Application', 'chrome.exe'),
      localAppData ? path.join(localAppData, 'Google', 'Chrome', 'Application', 'chrome.exe') : null,
      path.join(programFiles, 'Microsoft', 'Edge', 'Application', 'msedge.exe'),
      path.join(programFilesX86, 'Microsoft', 'Edge', 'Application', 'msedge.exe'),
      path.join(programFiles, 'Chromium', 'Application', 'chrome.exe'),
      path.join(programFiles, 'BraveSoftware', 'Brave-Browser', 'Application', 'brave.exe')
    );
  } else if (process.platform === 'darwin') {
    candidates.push(
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      '/Applications/Chromium.app/Contents/MacOS/Chromium',
      '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
      '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
      '/Applications/Arc.app/Contents/MacOS/Arc'
    );
  } else {
    candidates.push(
      '/usr/bin/google-chrome',
      '/usr/bin/google-chrome-stable',
      '/usr/bin/chromium',
      '/usr/bin/chromium-browser',
      '/snap/bin/chromium',
      '/usr/bin/microsoft-edge'
    );
  }
  for (const candidate of candidates) {
    if (isFile(candidate)) {
      return candidate;
    }
  }

  const names = [
    'google-chrome',
    'google-chrome-stable',
    'chromium',
    'chromium-browser',
    'microsoft-edge',
    'msedge',
    'chrome',
    'brave-browser',
    'brave',
  ];
  for (const name of names) {
    const found = which(name);
    if (found) {
      return found;
    }
  }
  return null;
}

// Best effort: the profile is throwaway, so a directory the browser still
// holds must never fail the render or change the `ok=`/`png=`/`warn=` contract.
function removeProfile(profile) {
  try {
    fs.rmSync(profile, { recursive: true, force: true });
  } catch (error) {
    // ignored on purpose
  }
}

function shoot(browser, htmlPath, pngPath, width, height, scale, wait) {
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'zedpolacode-profile-'));
  const common = [
    '--disable-gpu',
    '--hide-scrollbars',
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-extensions',
    '--disable-background-networking',
    '--disable-sync',
    '--mute-audio',
    '--default-background-color=00000000',
    '--force-device-scale-factor=' + scale,
    '--window-size=' + width + ',' + height,
    '--virtual-time-budget=' + wait,
    '--screenshot=' + pngPath,
    fileUrl(htmlPath),
  ];

  let detail = '';
  try {
    for (const mode of ['--headless=new', '--headless']) {
      try {
        if (fs.existsSync(pngPath)) {
          fs.unlinkSync(pngPath);
        }
      } catch (error) {
        // ignore
      }
      const run = spawnSync(browser, [mode, '--user-data-dir=' + profile].concat(common), {
        timeout: 60000,
        windowsHide: true,
        encoding: 'utf8',
        maxBuffer: 8 * 1024 * 1024,
      });
      if (isFile(pngPath) && fs.statSync(pngPath).size > 0) {
        return { ok: true, detail: '' };
      }
      const stderr = (run && run.stderr) || '';
      const failure = run && run.error ? ' ' + run.error.message : '';
      detail = (stderr + failure).trim().replace(/\s+/g, ' ').slice(-300);
    }
    return { ok: false, detail };
  } finally {
    // Gone before `main` emits anything, on success, failure and after the retry.
    removeProfile(profile);
  }
}

function main() {
  const raw = process.argv.slice(1).find((value) => String(value).trim().startsWith('{'));
  if (!raw) {
    emit(['error=missing-arguments']);
    return;
  }

  let spec;
  try {
    spec = JSON.parse(raw);
  } catch (error) {
    emit(['error=bad-arguments']);
    return;
  }

  const warnings = [];
  const outDir = spec.useTmp ? path.join(os.tmpdir(), 'zedpolacode') : spec.outDir;
  try {
    fs.mkdirSync(outDir, { recursive: true });
  } catch (error) {
    emit(['error=cannot-create-output-directory']);
    return;
  }

  const htmlPath = path.join(outDir, spec.base + '.html');
  try {
    if (spec.mode === 'file') {
      fs.copyFileSync(spec.html, htmlPath);
    } else {
      fs.writeFileSync(htmlPath, spec.html, 'utf8');
    }
  } catch (error) {
    emit(['error=cannot-write-html']);
    return;
  }

  let pngPath = null;
  let browserPath = null;

  if (spec.png) {
    const browser = findBrowser();
    if (!browser) {
      warnings.push('no-browser');
    } else {
      browserPath = browser;
      const target = path.join(outDir, spec.base + '.png');
      const shot = shoot(
        browser,
        htmlPath,
        target,
        spec.w,
        spec.h,
        spec.scale || 2,
        spec.wait || 1200
      );
      if (shot.ok) {
        pngPath = target;
      } else {
        warnings.push('render-failed' + (shot.detail ? ' (' + shot.detail + ')' : ''));
      }
    }
  }

  emit([
    'ok=1',
    'html=' + htmlPath,
    pngPath ? 'png=' + pngPath : '',
    browserPath ? 'browser=' + browserPath : '',
    warnings.length ? 'warn=' + warnings.join('; ') : '',
  ]);
}

main();
