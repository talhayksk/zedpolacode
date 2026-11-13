//! Chromium discovery and one controlled screenshot.
//!
//! The Zed extension does this from the host with Node (`src/zedpolacode.js`);
//! the MCP server is already a host process, so it does the same thing itself:
//! find a browser, write the card HTML into a throwaway directory, ask the
//! browser for a `--screenshot`, read the PNG back and delete the directory.
//!
//! `std::process::Command` has no timeout, so the child is polled and killed
//! after [`TIMEOUT`]. Nothing here runs on a normal `cargo test`: the one test
//! that launches a browser is `#[ignore]`d.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::card::{PngRenderer, RenderError};

const TIMEOUT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(50);
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Environment variables that can name a browser explicitly, in priority order.
const EXPLICIT_ENV: &[&str] = &[
    "POLACODE_BROWSER",
    "ZEDPOLACODE_BROWSER",
    "CHROME_PATH",
    "PUPPETEER_EXECUTABLE_PATH",
];

/// Executables looked up on `PATH`, after the fixed install locations.
const PATH_NAMES: &[&str] = &[
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "microsoft-edge",
    "msedge",
    "chrome",
    "brave-browser",
    "brave",
];

#[cfg(windows)]
const EXE_SUFFIX: &str = ".exe";
#[cfg(not(windows))]
const EXE_SUFFIX: &str = "";

#[cfg(windows)]
const PATH_SEPARATOR: char = ';';
#[cfg(not(windows))]
const PATH_SEPARATOR: char = ':';

/// Renders through a headless Chromium-based browser.
pub struct BrowserRenderer {
    browser: Option<PathBuf>,
}

impl BrowserRenderer {
    /// Finds a browser once, at startup, so a call never pays for discovery.
    pub fn discover() -> Self {
        Self {
            browser: find_browser(),
        }
    }

    /// Uses exactly this browser, or none. Tests rely on this to reach the
    /// "no browser" branch without touching the environment.
    #[cfg(test)]
    pub fn with_browser(browser: Option<PathBuf>) -> Self {
        Self { browser }
    }
}

impl PngRenderer for BrowserRenderer {
    fn render_png(
        &self,
        html: &str,
        width: u32,
        height: u32,
        scale: u32,
    ) -> Result<Vec<u8>, RenderError> {
        let browser = self.browser.as_deref().ok_or(RenderError::NoBrowser)?;
        shoot(browser, html, width, height, scale).map_err(RenderError::Failed)
    }
}

pub fn find_browser() -> Option<PathBuf> {
    let explicit: Vec<Option<String>> = EXPLICIT_ENV
        .iter()
        .map(|name| std::env::var(name).ok())
        .collect();
    let path = std::env::var("PATH").ok();
    let candidates = platform_candidates();

    resolve_browser(&explicit, &candidates, path.as_deref())
}

/// Shared by [`find_browser`] and its tests: explicit environment values, then
/// fixed install locations, then `PATH`.
fn resolve_browser(
    explicit: &[Option<String>],
    candidates: &[PathBuf],
    path: Option<&str>,
) -> Option<PathBuf> {
    for value in explicit.iter().flatten() {
        let candidate = PathBuf::from(value);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate.clone());
        }
    }

    let path = path?;
    for directory in path.split(PATH_SEPARATOR).filter(|part| !part.is_empty()) {
        for name in PATH_NAMES {
            let candidate = Path::new(directory).join(format!("{name}{EXE_SUFFIX}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(windows)]
fn platform_candidates() -> Vec<PathBuf> {
    let program_files =
        std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
    let program_files_x86 = std::env::var("ProgramFiles(x86)")
        .unwrap_or_else(|_| "C:\\Program Files (x86)".to_string());
    let local_app_data = std::env::var("LOCALAPPDATA").ok();

    let mut candidates = vec![
        Path::new(&program_files).join("Google/Chrome/Application/chrome.exe"),
        Path::new(&program_files_x86).join("Google/Chrome/Application/chrome.exe"),
    ];
    if let Some(local) = local_app_data {
        candidates.push(Path::new(&local).join("Google/Chrome/Application/chrome.exe"));
    }
    candidates.extend([
        Path::new(&program_files).join("Microsoft/Edge/Application/msedge.exe"),
        Path::new(&program_files_x86).join("Microsoft/Edge/Application/msedge.exe"),
        Path::new(&program_files).join("Chromium/Application/chrome.exe"),
        Path::new(&program_files).join("BraveSoftware/Brave-Browser/Application/brave.exe"),
    ]);
    candidates
}

#[cfg(target_os = "macos")]
fn platform_candidates() -> Vec<PathBuf> {
    [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        "/Applications/Arc.app/Contents/MacOS/Arc",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_candidates() -> Vec<PathBuf> {
    [
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/snap/bin/chromium",
        "/usr/bin/microsoft-edge",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

/// Screenshots `html` and returns the PNG bytes.
fn shoot(
    browser: &Path,
    html: &str,
    width: u32,
    height: u32,
    scale: u32,
) -> Result<Vec<u8>, String> {
    let dir = TempDir::create()?;
    let html_path = dir.path.join("card.html");
    std::fs::write(&html_path, html)
        .map_err(|error| format!("could not write the temporary HTML: {error}"))?;

    let png_path = dir.path.join("card.png");
    let profile = dir.path.join("profile");
    let log_path = dir.path.join("browser.log");
    let url = file_url(&html_path);

    let mut detail = String::new();
    // `--headless` on its own still works on older builds; retrying costs one
    // process spawn and rescues exactly the machines where `new` is missing.
    for mode in ["--headless=new", "--headless"] {
        let _ = std::fs::remove_file(&png_path);

        if let Err(error) = run_browser(
            browser, mode, &profile, &url, &png_path, &log_path, width, height, scale,
        ) {
            detail = error;
        }

        match std::fs::read(&png_path) {
            Ok(bytes) if !bytes.is_empty() && bytes.starts_with(PNG_SIGNATURE) => return Ok(bytes),
            Ok(bytes) if bytes.is_empty() => detail = "the browser wrote an empty PNG".to_string(),
            Ok(_) => detail = "the browser wrote something that is not a PNG".to_string(),
            Err(_) if detail.is_empty() => {
                detail = clip(&read_log(&log_path).unwrap_or_default(), 300)
            }
            Err(_) => {}
        }
    }

    if detail.is_empty() {
        detail = "the browser produced no PNG".to_string();
    }
    Err(detail)
}

#[allow(clippy::too_many_arguments)]
fn run_browser(
    browser: &Path,
    mode: &str,
    profile: &Path,
    url: &str,
    png_path: &Path,
    log_path: &Path,
    width: u32,
    height: u32,
    scale: u32,
) -> Result<(), String> {
    let log = std::fs::File::create(log_path)
        .map_err(|error| format!("could not create the browser log: {error}"))?;

    let mut command = Command::new(browser);
    command
        .arg(mode)
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("--disable-gpu")
        .arg("--hide-scrollbars")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-extensions")
        .arg("--disable-background-networking")
        .arg("--disable-sync")
        .arg("--mute-audio")
        .arg("--default-background-color=00000000")
        .arg(format!("--force-device-scale-factor={scale}"))
        .arg(format!("--window-size={width},{height}"))
        .arg("--virtual-time-budget=1200")
        .arg(format!("--screenshot={}", png_path.display()))
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: never flash a console window at the user.
        command.creation_flags(0x0800_0000);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("could not start `{}`: {error}", browser.display()))?;

    wait_for_child(&mut child)
}

/// Waits for `child`, killing it after [`TIMEOUT`].
fn wait_for_child(child: &mut std::process::Child) -> Result<(), String> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(error) => return Err(format!("could not wait for the browser: {error}")),
        }

        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "the browser timed out after {} seconds",
                TIMEOUT.as_secs()
            ));
        }

        std::thread::sleep(POLL);
    }
}

/// Runs the browser with `--dump-dom` on `html_path` and returns what it loaded.
///
/// Test evidence only: a `--screenshot` run writes a PNG even when the URL never
/// loaded (Chrome falls back to the new-tab page), so the e2e test asks the same
/// browser for the DOM behind the same URL and checks for the card markup.
#[cfg(test)]
pub fn dump_dom(browser: &Path, html_path: &Path) -> Result<String, String> {
    let dir = TempDir::create()?;
    let stdout_path = dir.path.join("dom.html");
    let log_path = dir.path.join("browser.log");
    let profile = dir.path.join("profile");
    let url = file_url(html_path);

    let mut detail = String::new();
    for mode in ["--headless=new", "--headless"] {
        let _ = std::fs::remove_file(&stdout_path);

        let log = std::fs::File::create(&log_path)
            .map_err(|error| format!("could not create the browser log: {error}"))?;
        // A file, not a pipe: Chrome's browser process outlives the launcher and
        // keeps writing to the inherited handle.
        let stdout = std::fs::File::create(&stdout_path)
            .map_err(|error| format!("could not create the browser stdout: {error}"))?;

        let mut command = Command::new(browser);
        command
            .arg(mode)
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--disable-gpu")
            .arg("--no-first-run")
            .arg("--disable-extensions")
            .arg("--virtual-time-budget=1200")
            .arg("--dump-dom")
            .arg(&url)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(log));

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }

        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start `{}`: {error}", browser.display()))?;

        if let Err(error) = wait_for_child(&mut child) {
            detail = error;
        }

        let dom = std::fs::read_to_string(&stdout_path).unwrap_or_default();
        if !dom.trim().is_empty() {
            return Ok(dom);
        }
        if detail.is_empty() {
            detail = clip(&read_log(&log_path).unwrap_or_default(), 300);
        }
    }

    if detail.is_empty() {
        detail = "the browser printed no DOM".to_string();
    }
    Err(detail)
}

fn read_log(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut clipped: String = text.chars().take(limit).collect();
    clipped.push('…');
    clipped
}

/// `file:///C:/path/card.html` for a drive or POSIX path.
///
/// Resolves the path first, because a relative path would otherwise produce a
/// URL that is not absolute. `canonicalize` can fail for a path that does not
/// exist yet; `absolute` still resolves it against the working directory.
fn file_url(path: &Path) -> String {
    let resolved = std::fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf());
    path_to_file_url(&resolved)
}

/// The pure half of [`file_url`]: a path in, a `file:` URL out, no I/O.
///
/// `canonicalize` returns a verbatim path on Windows (`\\?\C:\…`), and Chrome
/// treats `file:////?/C:/…` as something other than a file URL — it silently
/// shows the new-tab page. The verbatim prefix is therefore removed before the
/// URL is built. A verbatim UNC path (`\\?\UNC\server\share`) becomes the
/// ordinary UNC path first, whose URL keeps the server as the host.
fn path_to_file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");

    let normalized = if let Some(rest) = text.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else if let Some(rest) = text.strip_prefix("//?/") {
        rest.to_string()
    } else {
        text
    };

    if normalized.starts_with("//") {
        // `\\server\share\card.html` → `file://server/share/card.html`.
        return format!("file:{}", encode_path(&normalized));
    }

    // Drive and POSIX paths are absolute, so exactly three slashes follow the
    // scheme: `file:///C:/…`, `file:///home/…`.
    let absolute = if normalized.starts_with('/') {
        normalized
    } else {
        format!("/{normalized}")
    };
    format!("file://{}", encode_path(&absolute))
}

/// Percent-encodes the characters that would otherwise change the meaning of a
/// `file:` URL. Spaces and `#` appear in real Windows profile paths.
fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for ch in path.chars() {
        match ch {
            '%' => out.push_str("%25"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3F"),
            ' ' => out.push_str("%20"),
            other => out.push(other),
        }
    }
    out
}

/// A directory under the system temp dir that deletes itself.
pub struct TempDir {
    pub path: PathBuf,
}

impl TempDir {
    pub fn create() -> Result<Self, String> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "polacode-mcp-{}-{}-{}",
            std::process::id(),
            nanos,
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path)
            .map_err(|error| format!("could not create a temporary directory: {error}"))?;
        Ok(Self { path })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::ONE_PIXEL_PNG;

    #[test]
    fn explicit_environment_wins() {
        let dir = TempDir::create().expect("temp dir");
        let browser = dir.path.join("custom-browser");
        std::fs::write(&browser, b"#!/bin/sh\n").expect("write");

        let resolved = resolve_browser(
            &[None, Some(browser.to_string_lossy().to_string())],
            &[],
            None,
        );
        assert_eq!(resolved.as_deref(), Some(browser.as_path()));
    }

    #[test]
    fn a_path_entry_is_found_after_the_fixed_locations() {
        let dir = TempDir::create().expect("temp dir");
        let browser = dir.path.join(format!("chromium{EXE_SUFFIX}"));
        std::fs::write(&browser, b"binary").expect("write");

        let resolved = resolve_browser(
            &[Some("nowhere/browser".to_string())],
            &[dir.path.join("missing-browser")],
            Some(dir.path.to_string_lossy().as_ref()),
        );
        assert_eq!(resolved.as_deref(), Some(browser.as_path()));
    }

    #[test]
    fn nothing_found_is_none() {
        let resolved = resolve_browser(&[Some("nowhere/browser".to_string())], &[], Some(""));
        assert_eq!(resolved, None);
    }

    #[test]
    fn builds_a_file_url() {
        let url = file_url(Path::new("C:\\tmp\\a b#c.html"));
        assert!(url.starts_with("file:///"), "{url}");
        assert!(url.ends_with("/a%20b%23c.html"), "{url}");
    }

    #[test]
    fn a_windows_verbatim_path_loses_the_prefix() {
        let url = path_to_file_url(Path::new(r"\\?\C:\Users\AYBU\AppData\Local\Temp\card.html"));
        assert_eq!(url, "file:///C:/Users/AYBU/AppData/Local/Temp/card.html");
        assert!(!url.contains("//?/"), "{url}");
        assert!(!url.contains("file:////"), "{url}");
    }

    #[test]
    fn a_verbatim_unc_path_becomes_a_unc_url() {
        let url = path_to_file_url(Path::new(r"\\?\UNC\server\share\card.html"));
        assert_eq!(url, "file://server/share/card.html");
        assert!(!url.contains('?'), "{url}");
    }

    #[test]
    fn a_posix_path_keeps_its_shape() {
        assert_eq!(
            path_to_file_url(Path::new("/home/runner/card.html")),
            "file:///home/runner/card.html"
        );
    }

    #[test]
    fn spaces_and_hashes_are_still_encoded() {
        let url = path_to_file_url(Path::new(r"C:\Users\A B\card #1.html"));
        assert_eq!(url, "file:///C:/Users/A%20B/card%20%231.html");
        assert!(!url.contains(' '), "{url}");
        assert!(!url.contains('#'), "{url}");
    }

    /// The regression guard for the real thing: `file_url` goes through
    /// `canonicalize`, which is what returned a verbatim path on Windows.
    #[test]
    fn a_real_file_url_has_no_verbatim_prefix() {
        let dir = TempDir::create().expect("temp dir");
        let path = dir.path.join("card.html");
        std::fs::write(&path, "<html></html>").expect("write");

        let url = file_url(&path);
        assert!(url.starts_with("file:///"), "{url}");
        assert!(!url.contains("//?/"), "{url}");
        assert!(url.ends_with("/card.html"), "{url}");
    }

    #[test]
    fn a_relative_path_becomes_an_absolute_url() {
        // `canonicalize` fails because the file does not exist; the fallback
        // still resolves the path, so the URL can never be `file:relative/...`.
        let url = file_url(Path::new("relative/card.html"));
        assert!(url.starts_with("file:///"), "{url}");
        assert!(url.ends_with("/relative/card.html"), "{url}");
    }

    #[test]
    fn a_temp_directory_deletes_itself() {
        let path = {
            let dir = TempDir::create().expect("temp dir");
            std::fs::write(dir.path.join("x.txt"), b"x").expect("write");
            dir.path.clone()
        };
        assert!(!path.exists());
    }

    #[test]
    fn a_failing_browser_is_a_failed_render_not_a_panic() {
        let dir = TempDir::create().expect("temp dir");
        let fake = dir.path.join(format!("fake-browser{EXE_SUFFIX}"));
        // An empty file is not a runnable program, so the spawn fails.
        std::fs::write(&fake, b"").expect("write");

        let renderer = BrowserRenderer::with_browser(Some(fake));
        let error = renderer
            .render_png("<html></html>", 10, 10, 2)
            .expect_err("no PNG expected");
        // The QA case: a file accepted by discovery that cannot run. The tool
        // result has to tell the user which setting to fix, not only the path.
        let message = error.message();
        assert!(matches!(error, RenderError::Failed(_)), "{error:?}");
        assert!(message.contains("POLACODE_BROWSER"), "{message}");
    }

    #[test]
    fn no_browser_is_reported_as_such() {
        let error = BrowserRenderer::with_browser(None)
            .render_png("<html></html>", 10, 10, 2)
            .expect_err("no browser");
        assert_eq!(error, RenderError::NoBrowser);
        assert!(error.message().contains("POLACODE_BROWSER"));
    }

    #[test]
    fn the_png_signature_covers_the_fixture() {
        assert!(ONE_PIXEL_PNG.starts_with(PNG_SIGNATURE));
        assert_eq!(ONE_PIXEL_PNG.len(), 67);
    }

    /// A `--screenshot` run writes a PNG even when the URL never loaded (Chrome
    /// falls back to the new-tab page), so a PNG cannot tell a card from a
    /// browser error. This asks the same browser which document the card HTML
    /// resolves to.
    #[test]
    #[ignore = "launches a real headless browser"]
    fn real_browser_loads_the_card_document() {
        let args = crate::tools::SnapshotArgs::decode(&serde_json::json!({
            "code": "fn main() {\n    println!(\"loaded\");\n}\n",
            "lang": "rust",
            "title": "mcp-e2e.rs",
        }))
        .expect("valid arguments");
        let card = crate::card::build(&args).expect("a card");

        let dir = TempDir::create().expect("temp dir");
        let path = dir.path.join("mcp-e2e.html");
        std::fs::write(&path, &card.html).expect("write");

        let browser = find_browser().expect("a Chromium-based browser");
        let dom = dump_dom(&browser, &path).expect("the browser printed a DOM");
        let head: String = dom.chars().take(300).collect();

        assert!(
            dom.contains("class=\"card\""),
            "the browser did not load the card URL; DOM starts with: {head}"
        );
        assert!(
            dom.contains("mcp-e2e.rs"),
            "the card title is missing from the DOM; DOM starts with: {head}"
        );

        println!("dom: {} chars, card and title found", dom.chars().count());
    }
}
