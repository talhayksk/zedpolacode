//! The `card` command: a second entry point in the same binary.
//!
//! With no arguments `zpolacode-mcp` is the stdio server, unchanged. With
//! arguments it is a small CLI that turns the clipboard (or a file) into a PNG
//! without a model in the loop:
//!
//! ```text
//! zpolacode-mcp card --clipboard --lang Rust --title demo.rs
//! zpolacode-mcp card --file src/main.rs --lines 10-24
//! ```
//!
//! Clipboard text is read from the operating system, never interpolated into a
//! command line: a Zed task that expanded `ZED_SELECTED_TEXT` into a shell
//! command would be an injection risk. Argument parsing produces the same JSON
//! argument object the `snapshot` tool takes, so both surfaces go through
//! [`SnapshotArgs::decode`] and [`crate::card::build`] and cannot drift.
//!
//! The clipboard reader and the PNG renderer are traits for the same reason the
//! MCP tool uses one: the whole path is testable without a browser or a real
//! clipboard.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::browser::BrowserRenderer;
use crate::card::{self, Card, PngRenderer};
use crate::tools::{self, SnapshotArgs};

pub(crate) const EXIT_OK: i32 = 0;
pub(crate) const EXIT_RUNTIME: i32 = 1;
pub(crate) const EXIT_USAGE: i32 = 2;

/// Default output directory, relative to the working directory. Hidden so a
/// keybinding run does not litter the project root.
const DEFAULT_OUT: &str = ".zedpolacode";

/// Whether a bad argument (exit 2) or a runtime failure (exit 1) is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrorKind {
    Usage,
    Runtime,
}

#[derive(Debug)]
pub(crate) struct CliError {
    pub(crate) kind: ErrorKind,
    pub(crate) message: String,
}

impl CliError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Usage,
            message: message.into(),
        }
    }

    fn runtime(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Runtime,
            message: message.into(),
        }
    }

    pub(crate) fn exit_code(&self) -> i32 {
        match self.kind {
            ErrorKind::Usage => EXIT_USAGE,
            ErrorKind::Runtime => EXIT_RUNTIME,
        }
    }
}

/// Reads the code to snapshot from the operating system.
///
/// One small trait so tests inject a fake reader and the real one stays a thin
/// `Command` call.
pub(crate) trait Clipboard {
    fn read(&self) -> Result<String, String>;
}

/// The real clipboard, through the platform's own tool.
pub(crate) struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn read(&self) -> Result<String, String> {
        read_system_clipboard()
    }
}

/// Runs `command` and returns its stdout as UTF-8 text.
fn capture(command: &mut Command, what: &str) -> Result<String, String> {
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("Could not read the clipboard with {what}: {error}"))?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            format!(
                "Could not read the clipboard with {what} (exit {}).",
                output.status
            )
        } else {
            format!("Could not read the clipboard with {what}: {detail}")
        });
    }

    String::from_utf8(output.stdout)
        .map_err(|_| format!("The clipboard text from {what} is not valid UTF-8."))
}

#[cfg(windows)]
fn read_system_clipboard() -> Result<String, String> {
    // The exact command the brief specifies: PowerShell, no profile, UTF-8 out.
    let mut command = Command::new("powershell");
    command.args([
        "-NoProfile",
        "-Command",
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-Clipboard -Raw",
    ]);
    capture(&mut command, "PowerShell Get-Clipboard")
}

#[cfg(target_os = "macos")]
fn read_system_clipboard() -> Result<String, String> {
    capture(&mut Command::new("pbpaste"), "pbpaste")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn read_system_clipboard() -> Result<String, String> {
    let mut wayland = Command::new("wl-paste");
    wayland.arg("--no-newline");
    match capture(&mut wayland, "wl-paste") {
        Ok(text) => Ok(text),
        Err(wayland_error) => {
            // X11 sessions have no `wl-paste`; `xclip` is the documented fallback.
            let mut xclip = Command::new("xclip");
            xclip.args(["-selection", "clipboard", "-o"]);
            capture(&mut xclip, "xclip")
                .map_err(|xclip_error| format!("{wayland_error} Then {xclip_error}"))
        }
    }
}

#[cfg(not(any(windows, unix)))]
fn read_system_clipboard() -> Result<String, String> {
    Err("Reading the clipboard is not supported on this platform.".to_string())
}

#[derive(Debug)]
pub(crate) enum Invocation {
    Help,
    Version,
    Card(CardArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    Clipboard,
    File(String),
}

/// The decoded command line of one `card` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CardArgs {
    pub(crate) source: Source,
    pub(crate) lines: Option<String>,
    pub(crate) lang: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) theme: Option<String>,
    pub(crate) bg: Option<String>,
    pub(crate) font_size: Option<u32>,
    pub(crate) padding: Option<u32>,
    pub(crate) radius: Option<u32>,
    pub(crate) scale: Option<u32>,
    pub(crate) no_line_numbers: bool,
    pub(crate) no_window: bool,
    pub(crate) no_shadow: bool,
    pub(crate) out: Option<String>,
    pub(crate) open: bool,
    pub(crate) json: bool,
}

impl CardArgs {
    fn out_dir(&self) -> String {
        self.out.clone().unwrap_or_else(|| DEFAULT_OUT.to_string())
    }
}

/// Parses everything after the program name. `Err` is always a usage error.
fn parse(argv: &[String]) -> Result<Invocation, CliError> {
    let Some(first) = argv.first() else {
        return Err(CliError::usage(
            "No command. Run `zpolacode-mcp --help` for usage.",
        ));
    };

    match first.as_str() {
        "--help" | "-h" => return Ok(Invocation::Help),
        "--version" | "-V" => return Ok(Invocation::Version),
        "card" => {}
        other => {
            return Err(CliError::usage(format!(
                "Unknown command `{other}`. Run `zpolacode-mcp --help` for usage."
            )))
        }
    }

    let mut source: Option<Source> = None;
    let mut lines = None;
    let mut lang = None;
    let mut title = None;
    let mut theme = None;
    let mut bg = None;
    let mut font_size = None;
    let mut padding = None;
    let mut radius = None;
    let mut scale = None;
    let mut no_line_numbers = false;
    let mut no_window = false;
    let mut no_shadow = false;
    let mut out = None;
    let mut open = false;
    let mut json = false;

    let mut index = 1usize;
    while index < argv.len() {
        let token = argv[index].as_str();
        match token {
            "--clipboard" => set_source(&mut source, Source::Clipboard)?,
            "--file" => {
                let path = take_value("--file", argv, &mut index)?;
                set_source(&mut source, Source::File(path))?;
            }
            "--lines" => lines = Some(take_value("--lines", argv, &mut index)?),
            "--lang" => lang = Some(take_value("--lang", argv, &mut index)?),
            "--title" => title = Some(take_value("--title", argv, &mut index)?),
            "--theme" => theme = Some(take_value("--theme", argv, &mut index)?),
            "--bg" => bg = Some(take_value("--bg", argv, &mut index)?),
            "--font-size" => font_size = Some(take_u32("--font-size", argv, &mut index)?),
            "--padding" => padding = Some(take_u32("--padding", argv, &mut index)?),
            "--radius" => radius = Some(take_u32("--radius", argv, &mut index)?),
            "--scale" => scale = Some(take_u32("--scale", argv, &mut index)?),
            "--no-line-numbers" => no_line_numbers = true,
            "--no-window" => no_window = true,
            "--no-shadow" => no_shadow = true,
            "--out" => out = Some(take_value("--out", argv, &mut index)?),
            "--open" => open = true,
            "--json" => json = true,
            // `card --help` is the usage text, exactly like `zpolacode-mcp
            // --help`: the command reported an unknown option before.
            "--help" | "-h" => return Ok(Invocation::Help),
            other => {
                return Err(CliError::usage(format!(
                    "Unknown option `{other}`. Run `zpolacode-mcp --help` for usage."
                )))
            }
        }
        index += 1;
    }

    let source = source.ok_or_else(|| CliError::usage("Pass `--clipboard` or `--file <path>`."))?;

    if lines.is_some() && !matches!(&source, Source::File(_)) {
        return Err(CliError::usage(
            "`--lines` only applies to `--file`; a clipboard selection starts at line 1.",
        ));
    }

    Ok(Invocation::Card(CardArgs {
        source,
        lines,
        lang,
        title,
        theme,
        bg,
        font_size,
        padding,
        radius,
        scale,
        no_line_numbers,
        no_window,
        no_shadow,
        out,
        open,
        json,
    }))
}

fn set_source(current: &mut Option<Source>, next: Source) -> Result<(), CliError> {
    if current.is_some() {
        return Err(CliError::usage(
            "Pass either `--clipboard` or `--file`, not both.",
        ));
    }
    *current = Some(next);
    Ok(())
}

/// Consumes the argument after `flag`. A token that starts with `--` is treated
/// as the next flag, so a forgotten value fails loudly instead of being eaten.
fn take_value(flag: &str, argv: &[String], index: &mut usize) -> Result<String, CliError> {
    *index += 1;
    match argv.get(*index) {
        Some(value) if !value.starts_with("--") => Ok(value.clone()),
        _ => Err(CliError::usage(format!(
            "`{flag}` needs a value. Run `zpolacode-mcp --help` for usage."
        ))),
    }
}

fn take_u32(flag: &str, argv: &[String], index: &mut usize) -> Result<u32, CliError> {
    let raw = take_value(flag, argv, index)?;
    raw.parse::<u32>()
        .map_err(|_| CliError::usage(format!("`{flag}` expects a whole number, got `{raw}`.")))
}

/// Zed reports languages as display names (`Rust`, `Shell Script`). Lowercase
/// and drop whitespace so they resolve against the core's ids and aliases
/// (`shellscript`). Anything unknown falls back to the file extension inside
/// `polacode_core::highlight::language_for`, then to `plaintext`.
fn normalize_lang(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('.')
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Turns CLI flags into the same JSON `arguments` object the `snapshot` tool
/// receives, then decodes it exactly like the tool does.
fn prepare(args: &CardArgs, clipboard: &dyn Clipboard) -> Result<SnapshotArgs, CliError> {
    let arguments = tool_arguments(args, clipboard)?;
    SnapshotArgs::decode(&arguments).map_err(|error| CliError::usage(error.message))
}

fn tool_arguments(args: &CardArgs, clipboard: &dyn Clipboard) -> Result<Value, CliError> {
    let mut object = serde_json::Map::new();

    match &args.source {
        Source::Clipboard => {
            let text = clipboard.read().map_err(CliError::runtime)?;
            if text.trim().is_empty() {
                return Err(CliError::runtime(
                    "Nothing to snapshot: the clipboard is empty. Select the code, press Ctrl+C, then run the keybinding again.",
                ));
            }
            object.insert("code".to_string(), Value::String(text));
        }
        Source::File(path) => {
            object.insert("path".to_string(), Value::String(path.clone()));
            if let Some(lines) = &args.lines {
                object.insert("lines".to_string(), Value::String(lines.clone()));
            }
        }
    }

    if let Some(lang) = &args.lang {
        object.insert("lang".to_string(), Value::String(normalize_lang(lang)));
    }
    if let Some(title) = &args.title {
        object.insert("title".to_string(), Value::String(title.clone()));
    }
    if let Some(theme) = &args.theme {
        object.insert("theme".to_string(), Value::String(theme.clone()));
    }
    if let Some(bg) = &args.bg {
        object.insert("bg".to_string(), Value::String(bg.clone()));
    }
    if let Some(value) = args.font_size {
        object.insert("font_size".to_string(), json!(value));
    }
    if let Some(value) = args.padding {
        object.insert("padding".to_string(), json!(value));
    }
    if let Some(value) = args.radius {
        object.insert("radius".to_string(), json!(value));
    }
    if let Some(value) = args.scale {
        object.insert("scale".to_string(), json!(value));
    }

    // Clipboard mode cannot know the real start line, so line numbers are off
    // unless the file mode default applies.
    let line_numbers = !(args.no_line_numbers || matches!(&args.source, Source::Clipboard));
    object.insert("line_numbers".to_string(), Value::Bool(line_numbers));
    if args.no_window {
        object.insert("window".to_string(), Value::Bool(false));
    }
    if args.no_shadow {
        object.insert("shadow".to_string(), Value::Bool(false));
    }

    object.insert("out_dir".to_string(), Value::String(args.out_dir()));
    object.insert("write_files".to_string(), Value::Bool(true));

    Ok(Value::Object(object))
}

/// Everything up to `Card`, on the exact code path the MCP tool uses.
fn build_card(args: &CardArgs, clipboard: &dyn Clipboard) -> Result<Card, CliError> {
    let snapshot = prepare(args, clipboard)?;
    card::build(&snapshot).map_err(CliError::runtime)
}

/// The result of a successful `card` run: the card plus the files it wrote.
#[derive(Debug)]
pub(crate) struct Executed {
    pub(crate) card: Card,
    pub(crate) written: tools::Written,
}

impl Executed {
    /// One short human line: path, canvas size, theme, line count.
    pub(crate) fn human(&self) -> String {
        let path = self
            .written
            .png
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "(the PNG was not written)".to_string());
        format!(
            "{path} ({}x{} px @{}x, {}, {} lines)",
            self.card.width.saturating_mul(self.card.scale),
            self.card.height.saturating_mul(self.card.scale),
            self.card.scale,
            self.card.theme,
            self.card.lines
        )
    }

    /// The MCP text summary's field set as a JSON object.
    pub(crate) fn json(&self) -> Value {
        tools::summary_json(&self.card, &self.written)
    }
}

fn execute(
    args: &CardArgs,
    renderer: &dyn PngRenderer,
    clipboard: &dyn Clipboard,
) -> Result<Executed, CliError> {
    let card = build_card(args, clipboard)?;
    let png = renderer
        .render_png(&card.html, card.width, card.height, card.scale)
        .map_err(|error| CliError::runtime(error.message()))?;

    let written = tools::write_files(&card, &png, &args.out_dir()).map_err(CliError::runtime)?;
    Ok(Executed { card, written })
}

/// Shows a finished card.
///
/// A trait because the interesting case is a viewer that keeps its file open
/// (Photos memory-maps the PNG on Windows), and a test must be able to act
/// like one without launching a GUI.
pub(crate) trait Viewer {
    fn open(&self, path: &Path) -> Result<(), String>;
}

/// The platform viewer: `start`, `open` or `xdg-open`.
struct SystemViewer;

impl Viewer for SystemViewer {
    fn open(&self, path: &Path) -> Result<(), String> {
        open_in_viewer(path)
    }
}

#[cfg(windows)]
fn viewer_command(path: &Path) -> Command {
    // `start` is a cmd builtin. The empty argument is the window title, so a
    // path is never mistaken for one.
    let mut command = Command::new("cmd");
    command.arg("/c").arg("start").arg("").arg(path);
    command
}

#[cfg(target_os = "macos")]
fn viewer_command(path: &Path) -> Command {
    let mut command = Command::new("open");
    command.arg(path);
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn viewer_command(path: &Path) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(path);
    command
}

fn open_in_viewer(path: &Path) -> Result<(), String> {
    viewer_command(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open `{}`: {error}", path.display()))
}

/// Where the throw-away copies the viewer opens live. Deliberately outside the
/// project: a viewer keeps the file it shows open (Photos memory-maps it on
/// Windows), and a memory-mapped file cannot be rewritten
/// (`ERROR_USER_MAPPED_FILE`, os error 1224). The project file must therefore
/// never be the file the viewer holds.
const VIEWER_DIR: &str = "zpolacode-mcp-viewer";

/// How many copies of one card base a viewer may hold at once. A new snapshot
/// reuses the first copy the viewer has closed; when every slot is still held
/// the CLI warns instead of opening a viewer, and names the directory.
const VIEWER_SLOTS: u32 = 16;

/// Copies `source` to a path the viewer can hold open without touching the
/// project file, and returns it.
///
/// The copy goes to `<temp>/zpolacode-mcp-viewer/<base>-<slot>.png`, slot 1
/// first. A copy the viewer has closed is deleted and reused, so the common
/// case keeps one file per card; a copy it still holds cannot be deleted or
/// overwritten, and the next slot is tried. All [`VIEWER_SLOTS`] slots held at
/// once is an error the caller reports as a warning.
fn viewer_copy(source: &Path) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(VIEWER_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("Could not create `{}`: {error}", dir.display()))?;

    let stem = source
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "card".to_string());

    let mut last = String::new();
    for slot in 1..=VIEWER_SLOTS {
        let copy = dir.join(format!("{stem}-{slot}.png"));
        // Removing a copy whose viewer has closed reuses the slot instead of
        // adding one. A copy the viewer still holds cannot be deleted on
        // Windows, so the copy to that path fails and the next slot is tried.
        let _ = std::fs::remove_file(&copy);
        match std::fs::copy(source, &copy) {
            Ok(_) => return Ok(copy),
            Err(error) => last = error.to_string(),
        }
    }

    Err(format!(
        "Could not copy the PNG for the viewer: all {VIEWER_SLOTS} copies of `{stem}` in `{}` are still open ({last})",
        dir.display()
    ))
}

fn report_error(error: &CliError) -> i32 {
    eprintln!("zpolacode-mcp: {}", error.message);
    error.exit_code()
}

fn run_card(
    args: &CardArgs,
    renderer: &dyn PngRenderer,
    clipboard: &dyn Clipboard,
    viewer: &dyn Viewer,
) -> i32 {
    let executed = match execute(args, renderer, clipboard) {
        Ok(executed) => executed,
        Err(error) => return report_error(&error),
    };

    if args.open {
        if let Some(path) = executed.written.png.as_deref() {
            // Best effort: the PNG is already on disk, so a missing viewer is a
            // warning, not a failed run.
            match viewer_copy(path) {
                Ok(copy) => {
                    if let Err(message) = viewer.open(&copy) {
                        eprintln!("zpolacode-mcp: {message}");
                    }
                }
                Err(message) => {
                    eprintln!(
                        "zpolacode-mcp: {message}; the PNG is at `{}`.",
                        path.display()
                    );
                }
            }
        }
    }

    if args.json {
        match serde_json::to_string(&executed.json()) {
            Ok(text) => println!("{text}"),
            Err(error) => {
                eprintln!("zpolacode-mcp: could not encode the summary: {error}");
                return EXIT_RUNTIME;
            }
        }
    } else {
        println!("{}", executed.human());
    }

    EXIT_OK
}

fn version_line() -> String {
    format!("zpolacode-mcp {}", env!("CARGO_PKG_VERSION"))
}

fn help_text() -> String {
    format!(
        r#"zpolacode-mcp {version} - turn code into a PNG code card.

With no arguments this binary is the MCP stdio server: one JSON-RPC object per
line on stdout. With `card` it is a small command-line tool that renders the
same card without a model in the loop.

Usage:
  zpolacode-mcp card --clipboard [options]
  zpolacode-mcp card --file <path> [--lines <range>] [options]
  zpolacode-mcp card --help
  zpolacode-mcp --help
  zpolacode-mcp --version

Source (exactly one):
  --clipboard              Read the code from the OS clipboard. Select the code
                           in the editor and press Ctrl+C first.
  --file <path>            Read the code from a file.
  --lines <range>          With --file: 40-70, 40:70, 40..70, 40+70, 40, or 40-

Card:
  --lang <id>              Language id or Zed language name (Rust, Python,
                           Shell Script, ...). Default: the file extension,
                           then plaintext.
  --title <text>           Title bar text. Default: the file name, or snippet.
  --theme <id>             dracula (default), one-dark, nord, tokyo-night,
                           github-dark, catppuccin-mocha, github-light,
                           solarized-light.
  --bg <name|#hex>         transparent (default), dark, light, gradient, sunset,
                           ocean, aurora, midnight, mono, or a hex colour.
  --font-size <px>         9-40, default 15.
  --padding <px>           0-120, default 34.
  --radius <px>            0-48, default 14.
  --scale <1-4>            Device pixel ratio, default 2.
  --no-line-numbers        Hide line numbers. Off by default for --clipboard,
                           on for --file.
  --no-window              Hide the title bar and window dots.
  --no-shadow              Do not draw the drop shadow.

Output:
  --out <dir>              Where to write the .png and .html. Default
                           .zedpolacode in the current directory.
  --open                   Open the PNG in the default viewer. The viewer gets
                           a copy in the system temp directory, so the project
                           file stays writable and always holds the newest card.
  --json                   Print one JSON object instead of a human line.

Other:
  --help                   Show this text.
  --version                Show the version.

Exit codes: 0 success, 2 bad arguments, 1 runtime failure (no clipboard, an
empty selection, no browser, an unwritable output directory). If no browser is
found, set POLACODE_BROWSER (or CHROME_PATH) to a Chrome, Edge, Chromium or
Brave executable.
"#,
        version = env!("CARGO_PKG_VERSION")
    )
}

/// The CLI entry point. Returns the process exit code; `main` exits with it.
pub(crate) fn main(argv: &[String]) -> i32 {
    match parse(argv) {
        Ok(Invocation::Help) => {
            print!("{}", help_text());
            EXIT_OK
        }
        Ok(Invocation::Version) => {
            println!("{}", version_line());
            EXIT_OK
        }
        Ok(Invocation::Card(args)) => {
            // Discovered only for `card`, so `--help` and `--version` stay cheap.
            let renderer = BrowserRenderer::discover();
            run_card(&args, &renderer, &SystemClipboard, &SystemViewer)
        }
        Err(error) => report_error(&error),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use super::*;
    use crate::browser::TempDir;
    use crate::card::RenderError;
    use crate::fixture::{FakeRenderer, ONE_PIXEL_PNG};

    struct FakeClipboard(&'static str);

    impl Clipboard for FakeClipboard {
        fn read(&self) -> Result<String, String> {
            Ok(self.0.to_string())
        }
    }

    struct BrokenClipboard;

    impl Clipboard for BrokenClipboard {
        fn read(&self) -> Result<String, String> {
            Err("no clipboard tool on this machine".to_string())
        }
    }

    struct NoBrowserRenderer;

    impl PngRenderer for NoBrowserRenderer {
        fn render_png(
            &self,
            _html: &str,
            _width: u32,
            _height: u32,
            _scale: u32,
        ) -> Result<Vec<u8>, RenderError> {
            Err(RenderError::NoBrowser)
        }
    }

    /// Opens `path` the way a viewer that keeps the file open does: on Windows
    /// with no sharing at all, so a write, rename or delete of that exact path
    /// fails until the handle is dropped. The defect this pins is therefore
    /// Windows-only (POSIX readers never block a writer).
    #[cfg(windows)]
    fn hold_open(path: &Path) -> Result<std::fs::File, String> {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(path)
            .map_err(|error| format!("could not hold `{}`: {error}", path.display()))
    }

    #[cfg(not(windows))]
    fn hold_open(path: &Path) -> Result<std::fs::File, String> {
        std::fs::File::open(path)
            .map_err(|error| format!("could not hold `{}`: {error}", path.display()))
    }

    /// A viewer that never lets go of what it was given.
    struct HoldingViewer {
        opened: Mutex<Vec<PathBuf>>,
        held: Mutex<Vec<std::fs::File>>,
    }

    impl HoldingViewer {
        fn new() -> Self {
            Self {
                opened: Mutex::new(Vec::new()),
                held: Mutex::new(Vec::new()),
            }
        }

        fn opened(&self) -> Vec<PathBuf> {
            self.opened.lock().expect("the viewer log").clone()
        }
    }

    impl Viewer for HoldingViewer {
        fn open(&self, path: &Path) -> Result<(), String> {
            let file = hold_open(path)?;
            self.held.lock().expect("the held handles").push(file);
            self.opened
                .lock()
                .expect("the viewer log")
                .push(path.to_path_buf());
            Ok(())
        }
    }

    /// A renderer that returns a distinct PNG per call, so a test can tell
    /// which snapshot ended up in the project file.
    struct RunRenderer(Mutex<Vec<Vec<u8>>>);

    impl RunRenderer {
        fn new() -> Self {
            Self(Mutex::new(Vec::new()))
        }

        fn renders(&self) -> Vec<Vec<u8>> {
            self.0.lock().expect("the render log").clone()
        }
    }

    impl PngRenderer for RunRenderer {
        fn render_png(
            &self,
            _html: &str,
            _width: u32,
            _height: u32,
            _scale: u32,
        ) -> Result<Vec<u8>, RenderError> {
            let mut renders = self.0.lock().expect("the render log");
            let mut png = ONE_PIXEL_PNG.to_vec();
            // Still starts with the PNG signature; the extra byte only makes
            // the runs distinguishable to the assertions.
            png.push(u8::try_from(renders.len() + 1).unwrap_or(1));
            renders.push(png.clone());
            Ok(png)
        }
    }

    fn parse_args(argv: &[&str]) -> Result<Invocation, CliError> {
        let owned: Vec<String> = argv.iter().map(|value| value.to_string()).collect();
        parse(&owned)
    }

    fn card_args(argv: &[&str]) -> CardArgs {
        match parse_args(argv) {
            Ok(Invocation::Card(args)) => args,
            other => panic!("expected card arguments, got {other:?}"),
        }
    }

    #[test]
    fn every_flag_parses() {
        let args = card_args(&[
            "card",
            "--clipboard",
            "--lang",
            "Rust",
            "--title",
            "demo.rs",
            "--theme",
            "nord",
            "--bg",
            "#1e1e2e",
            "--font-size",
            "18",
            "--padding",
            "20",
            "--radius",
            "10",
            "--scale",
            "3",
            "--no-line-numbers",
            "--no-window",
            "--no-shadow",
            "--out",
            "shots",
            "--open",
            "--json",
        ]);

        assert_eq!(args.source, Source::Clipboard);
        assert_eq!(args.lang.as_deref(), Some("Rust"));
        assert_eq!(args.title.as_deref(), Some("demo.rs"));
        assert_eq!(args.theme.as_deref(), Some("nord"));
        assert_eq!(args.bg.as_deref(), Some("#1e1e2e"));
        assert_eq!(args.font_size, Some(18));
        assert_eq!(args.padding, Some(20));
        assert_eq!(args.radius, Some(10));
        assert_eq!(args.scale, Some(3));
        assert!(args.no_line_numbers);
        assert!(args.no_window);
        assert!(args.no_shadow);
        assert_eq!(args.out.as_deref(), Some("shots"));
        assert!(args.open);
        assert!(args.json);
    }

    #[test]
    fn a_file_and_a_line_range_parse() {
        let args = card_args(&["card", "--file", "src/main.rs", "--lines", "10-24"]);
        assert_eq!(args.source, Source::File("src/main.rs".to_string()));
        assert_eq!(args.lines.as_deref(), Some("10-24"));
        assert!(!args.json);
    }

    #[test]
    fn clipboard_and_file_are_exclusive() {
        let error = parse_args(&["card", "--clipboard", "--file", "a.rs"]).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Usage);
        assert!(error.message.contains("--clipboard"), "{}", error.message);
        assert!(error.message.contains("--file"), "{}", error.message);
    }

    #[test]
    fn a_source_is_required() {
        let error = parse_args(&["card"]).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Usage);
        assert!(error.message.contains("--clipboard"), "{}", error.message);
    }

    #[test]
    fn a_missing_value_names_the_flag() {
        let error = parse_args(&["card", "--clipboard", "--lang"]).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Usage);
        assert!(error.message.contains("--lang"), "{}", error.message);
    }

    #[test]
    fn an_unknown_flag_is_rejected() {
        let error = parse_args(&["card", "--clipboard", "--colour", "red"]).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Usage);
        assert!(error.message.contains("--colour"), "{}", error.message);
    }

    #[test]
    fn an_unknown_command_is_rejected() {
        let error = parse_args(&["render"]).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Usage);
        assert!(error.message.contains("render"), "{}", error.message);
    }

    #[test]
    fn lines_require_a_file() {
        let error = parse_args(&["card", "--clipboard", "--lines", "2-3"]).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Usage);
        assert!(error.message.contains("--lines"), "{}", error.message);
    }

    #[test]
    fn a_font_size_outside_the_range_is_usage() {
        let args = card_args(&["card", "--clipboard", "--font-size", "41"]);
        let error = prepare(&args, &FakeClipboard("fn main() {}")).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Usage);
        assert!(error.message.contains("font_size"), "{}", error.message);
    }

    #[test]
    fn a_scale_outside_the_range_is_usage() {
        for value in ["0", "5"] {
            let args = card_args(&["card", "--clipboard", "--scale", value]);
            let error = prepare(&args, &FakeClipboard("x")).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Usage, "scale {value}");
            assert!(error.message.contains("scale"), "{}", error.message);
        }
    }

    #[test]
    fn an_unknown_theme_is_usage() {
        let args = card_args(&["card", "--clipboard", "--theme", "nope"]);
        let error = prepare(&args, &FakeClipboard("x")).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Usage);
        assert!(error.message.contains("nope"), "{}", error.message);
    }

    #[test]
    fn a_non_numeric_size_is_usage() {
        let error = parse_args(&["card", "--clipboard", "--font-size", "big"]).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Usage);
        assert!(error.message.contains("--font-size"), "{}", error.message);
    }

    #[test]
    fn lang_is_normalised_like_zed_reports_it() {
        assert_eq!(normalize_lang("Rust"), "rust");
        assert_eq!(normalize_lang("  Python "), "python");
        assert_eq!(normalize_lang("Shell Script"), "shellscript");
        assert_eq!(normalize_lang(".TypeScript"), "typescript");

        let args = card_args(&[
            "card",
            "--clipboard",
            "--lang",
            "Shell Script",
            "--title",
            "run.sh",
        ]);
        let card = build_card(&args, &FakeClipboard("echo hi")).expect("card");
        assert_eq!(card.language, "shell");

        let args = card_args(&[
            "card",
            "--clipboard",
            "--lang",
            "Rust",
            "--title",
            "demo.rs",
        ]);
        let card = build_card(&args, &FakeClipboard("fn main() {}")).expect("card");
        assert_eq!(card.language, "rust");
    }

    #[test]
    fn an_unknown_language_falls_back_to_the_extension_then_plaintext() {
        let args = card_args(&[
            "card",
            "--clipboard",
            "--lang",
            "Brainfuck",
            "--title",
            "notes.py",
        ]);
        let card = build_card(&args, &FakeClipboard("print(1)")).expect("card");
        assert_eq!(card.language, "python");

        let args = card_args(&["card", "--clipboard", "--lang", "Brainfuck"]);
        let card = build_card(&args, &FakeClipboard("hello")).expect("card");
        assert_eq!(card.language, "plaintext");
    }

    #[test]
    fn clipboard_text_becomes_a_card_without_line_numbers() {
        // CRLF and Turkish characters are exactly what Windows + Zed produce.
        let clipboard = FakeClipboard(
            "fn main() {\r\n    let selamlama = \"merhaba ğüşi\";\r\n\r\n    println!(\"{selamlama}\");\r\n}\r\n",
        );
        let args = card_args(&[
            "card",
            "--clipboard",
            "--lang",
            "Rust",
            "--title",
            "demo.rs",
        ]);
        let card = build_card(&args, &clipboard).expect("card");

        assert!(
            card.html.contains("merhaba ğüşi"),
            "Turkish text is preserved"
        );
        assert!(!card.html.contains('\r'), "CRLF was normalised away");
        assert!(
            !card.html.contains("class=\"ln\""),
            "clipboard mode hides line numbers"
        );
        assert!(!card.html.contains(".ln{"), "no line-number CSS either");
        assert_eq!(card.lines, 5);
        assert_eq!(card.first_line, 1);
        assert_eq!(card.language, "rust");
        assert_eq!(card.title, "demo.rs");
    }

    #[test]
    fn file_mode_keeps_line_numbers() {
        let dir = TempDir::create().expect("temp dir");
        let path = dir.path.join("demo.rs");
        std::fs::write(&path, "fn main() {\n    run();\n}\n").expect("write");
        let path_text = path.to_string_lossy().to_string();

        let args = card_args(&["card", "--file", path_text.as_str(), "--lines", "1-2"]);
        let card = build_card(&args, &FakeClipboard("")).expect("card");
        assert!(
            card.html.contains("class=\"ln\""),
            "file mode keeps line numbers"
        );
        assert_eq!(card.lines, 2);
        assert_eq!(card.first_line, 1);
        assert_eq!(card.language, "rust");

        let args = card_args(&["card", "--file", path_text.as_str(), "--no-line-numbers"]);
        let card = build_card(&args, &FakeClipboard("")).expect("card");
        assert!(
            !card.html.contains("class=\"ln\""),
            "--no-line-numbers wins in file mode"
        );
    }

    #[test]
    fn an_empty_clipboard_is_a_runtime_error_and_writes_nothing() {
        let dir = TempDir::create().expect("temp dir");
        let out = dir.path.join("out");
        let out_text = out.to_string_lossy().to_string();

        for text in ["", "   \r\n\t"] {
            let args = card_args(&["card", "--clipboard", "--out", out_text.as_str()]);
            let error = execute(&args, &FakeRenderer, &FakeClipboard(text)).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Runtime, "{text:?}");
            assert!(
                error.message.contains("clipboard is empty"),
                "{}",
                error.message
            );
            assert!(error.message.contains("Ctrl+C"), "{}", error.message);
            assert!(!out.exists(), "no output for {text:?}");
        }
    }

    #[test]
    fn a_broken_clipboard_tool_is_a_runtime_error() {
        let args = card_args(&["card", "--clipboard"]);
        let error = execute(&args, &FakeRenderer, &BrokenClipboard).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Runtime);
        assert!(
            error.message.contains("no clipboard tool"),
            "{}",
            error.message
        );
    }

    #[test]
    fn json_output_uses_the_mcp_summary_fields() {
        let dir = TempDir::create().expect("temp dir");
        let out = dir.path.join("out");
        let out_text = out.to_string_lossy().to_string();
        let args = card_args(&[
            "card",
            "--clipboard",
            "--lang",
            "Rust",
            "--title",
            "demo.rs",
            "--out",
            out_text.as_str(),
            "--json",
        ]);

        let executed = execute(&args, &FakeRenderer, &FakeClipboard("fn main() {}")).expect("card");
        let summary = executed.json();

        assert_eq!(summary["lines"], json!(1));
        assert_eq!(summary["first_line"], json!(1));
        assert_eq!(summary["language"], json!("rust"));
        assert_eq!(summary["theme"], json!("dracula"));
        assert_eq!(summary["title"], json!("demo.rs"));
        assert!(summary["canvas_css"].is_string(), "{summary}");
        assert!(summary["canvas_device"].is_string(), "{summary}");
        assert!(
            summary["png"].as_str().unwrap_or("").ends_with("demo.png"),
            "{summary}"
        );
        assert!(
            summary["html"]
                .as_str()
                .unwrap_or("")
                .ends_with("demo.html"),
            "{summary}"
        );
        assert_eq!(
            executed.written.png.as_deref(),
            Some(out.join("demo.png").as_path())
        );
        assert_eq!(
            std::fs::read(out.join("demo.png")).expect("png"),
            ONE_PIXEL_PNG
        );

        // The `--json` object and the MCP text summary must expose one key set.
        let text = tools::summary(&executed.card, &executed.written);
        let text_keys: BTreeSet<&str> = text
            .lines()
            .filter_map(|line| line.split_once(": ").map(|(key, _)| key))
            .collect();
        let json_keys: BTreeSet<&str> = summary
            .as_object()
            .map(|object| object.keys().map(String::as_str).collect())
            .unwrap_or_default();
        assert_eq!(text_keys, json_keys);
    }

    #[test]
    fn the_human_line_names_the_file_size_theme_and_lines() {
        let dir = TempDir::create().expect("temp dir");
        let out = dir.path.join("out");
        let out_text = out.to_string_lossy().to_string();
        let args = card_args(&[
            "card",
            "--clipboard",
            "--title",
            "demo.rs",
            "--out",
            out_text.as_str(),
        ]);

        let executed = execute(&args, &FakeRenderer, &FakeClipboard("fn main() {}")).expect("card");
        let line = executed.human();
        assert!(line.contains("demo.png"), "{line}");
        assert!(line.contains("dracula"), "{line}");
        assert!(line.contains("1 lines"), "{line}");
        assert!(line.contains("@2x"), "{line}");
        assert!(!line.contains('\n'), "one line only: {line:?}");
    }

    #[test]
    fn a_missing_browser_names_the_environment_variable() {
        let dir = TempDir::create().expect("temp dir");
        let out = dir.path.join("out");
        let out_text = out.to_string_lossy().to_string();
        let args = card_args(&["card", "--clipboard", "--out", out_text.as_str()]);

        let error = execute(&args, &NoBrowserRenderer, &FakeClipboard("fn main() {}")).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Runtime);
        assert!(
            error.message.contains("POLACODE_BROWSER"),
            "{}",
            error.message
        );
        assert!(!out.exists(), "no output on a failed render");
        assert_eq!(error.exit_code(), EXIT_RUNTIME);
    }

    #[test]
    fn a_second_snapshot_succeeds_while_the_viewer_holds_the_first() {
        let dir = TempDir::create().expect("temp dir");
        let out = dir.path.join("out");
        let out_text = out.to_string_lossy().to_string();
        let args = card_args(&[
            "card",
            "--clipboard",
            "--title",
            "demo.rs",
            "--out",
            out_text.as_str(),
            "--open",
        ]);
        let renderer = RunRenderer::new();
        let viewer = HoldingViewer::new();

        let first = run_card(&args, &renderer, &FakeClipboard("fn main() {}"), &viewer);
        assert_eq!(first, EXIT_OK, "the first snapshot renders");

        let second = run_card(
            &args,
            &renderer,
            &FakeClipboard("fn main() { second(); }"),
            &viewer,
        );
        assert_eq!(
            second, EXIT_OK,
            "the second snapshot must not fail while the viewer holds the first"
        );

        // The project file holds the latest snapshot, not the one the viewer
        // is showing.
        let renders = renderer.renders();
        assert_eq!(renders.len(), 2, "both runs rendered");
        assert_eq!(
            std::fs::read(out.join("demo.png")).expect("the project PNG"),
            renders[1]
        );

        // The viewer was handed copies outside the project, one per snapshot.
        let opened = viewer.opened();
        assert_eq!(opened.len(), 2, "the viewer opened both snapshots");
        if cfg!(windows) {
            // A held copy cannot be deleted or replaced on Windows, so the
            // second snapshot needs its own file. POSIX lets a reader share
            // the file, so there slot 1 is reused and the paths are equal.
            assert_ne!(opened[0], opened[1], "each snapshot needs its own copy");
        }
        for path in &opened {
            assert!(
                !path.starts_with(&out),
                "the viewer must never map the project file: {}",
                path.display()
            );
            assert_eq!(
                path.extension().and_then(|value| value.to_str()),
                Some("png"),
                "{}",
                path.display()
            );
            assert!(path.exists(), "{} is still available", path.display());
        }

        // Test hygiene: drop the "viewer" handles, then its copies.
        drop(viewer);
        for path in &opened {
            let _ = std::fs::remove_file(path);
        }
        let _ = std::fs::remove_dir(std::env::temp_dir().join(VIEWER_DIR));
    }

    #[test]
    #[cfg(windows)]
    fn viewer_copies_are_bounded_and_reused() {
        let dir = TempDir::create().expect("temp dir");
        let source = dir.path.join("polacode-viewer-slot.png");
        std::fs::write(&source, b"card").expect("write");

        let first = viewer_copy(&source).expect("a copy");
        assert_eq!(
            first.file_name().and_then(|name| name.to_str()),
            Some("polacode-viewer-slot-1.png"),
            "the first free slot is used"
        );

        let held = hold_open(&first).expect("hold the copy");
        let second = viewer_copy(&source).expect("a second copy");
        assert_ne!(first, second, "a copy the viewer holds is skipped");
        assert_eq!(
            second.file_name().and_then(|name| name.to_str()),
            Some("polacode-viewer-slot-2.png")
        );
        drop(held);

        let reused = viewer_copy(&source).expect("a reused slot");
        assert_eq!(reused, first, "a copy the viewer closed is reused");

        // Every slot held at once is the documented bound.
        let mut held = Vec::new();
        let mut copies = Vec::new();
        for _ in 0..VIEWER_SLOTS {
            let copy = viewer_copy(&source).expect("a free slot");
            held.push(hold_open(&copy).expect("hold the copy"));
            copies.push(copy);
        }
        let error = viewer_copy(&source).expect_err("no slot left");
        assert!(error.contains(&VIEWER_SLOTS.to_string()), "{error}");
        assert!(error.contains(VIEWER_DIR), "{error}");

        // Test hygiene: release the handles, then remove the copies.
        drop(held);
        for copy in copies {
            let _ = std::fs::remove_file(copy);
        }
        let _ = std::fs::remove_file(reused);
        let _ = std::fs::remove_dir(std::env::temp_dir().join(VIEWER_DIR));
    }

    #[test]
    fn card_help_is_help_not_an_unknown_option() {
        assert!(matches!(
            parse_args(&["card", "--help"]),
            Ok(Invocation::Help)
        ));
        assert!(matches!(parse_args(&["card", "-h"]), Ok(Invocation::Help)));
        assert!(matches!(
            parse_args(&["card", "--clipboard", "--help"]),
            Ok(Invocation::Help)
        ));
        // A bad flag is still a usage error, not a help request.
        assert_eq!(
            parse_args(&["card", "--badflag"]).unwrap_err().exit_code(),
            EXIT_USAGE
        );
    }

    #[test]
    fn usage_errors_exit_two() {
        assert_eq!(parse_args(&["card"]).unwrap_err().exit_code(), EXIT_USAGE);
    }

    #[test]
    fn help_and_version_short_circuit() {
        assert!(matches!(parse_args(&["--help"]), Ok(Invocation::Help)));
        assert!(matches!(
            parse_args(&["--version"]),
            Ok(Invocation::Version)
        ));

        let help = help_text();
        for flag in [
            "--clipboard",
            "--file",
            "--lines",
            "--lang",
            "--title",
            "--theme",
            "--bg",
            "--font-size",
            "--padding",
            "--radius",
            "--scale",
            "--no-line-numbers",
            "--no-window",
            "--no-shadow",
            "--out",
            "--open",
            "--json",
            "--help",
            "--version",
        ] {
            assert!(help.contains(flag), "help omits `{flag}`");
        }
        assert!(
            help.contains("POLACODE_BROWSER"),
            "help names the browser knob"
        );

        assert!(version_line().contains(env!("CARGO_PKG_VERSION")));
    }

    /// The only CLI test that needs a real browser: build the card from a real
    /// file, read the HTML it wrote, and capture a real PNG from that document.
    #[test]
    #[ignore = "launches a real headless browser"]
    fn real_browser_renders_the_saved_html() {
        let dir = TempDir::create().expect("temp dir");
        let source = dir.path.join("demo.rs");
        std::fs::write(
            &source,
            "fn main() {\n    let selamlama = \"merhaba ğüşi\";\n\n    println!(\"{selamlama}\");\n}\n",
        )
        .expect("write");
        let out = dir.path.join("out");

        let source_text = source.to_string_lossy().to_string();
        let out_text = out.to_string_lossy().to_string();
        let args = card_args(&[
            "card",
            "--file",
            source_text.as_str(),
            "--title",
            "real.rs",
            "--out",
            out_text.as_str(),
        ]);

        let renderer = BrowserRenderer::discover();
        let executed = execute(&args, &renderer, &FakeClipboard("")).expect("real render");

        let html_path = executed
            .written
            .html
            .as_deref()
            .expect("the HTML file was written");
        let html = std::fs::read_to_string(html_path).expect("read the HTML back");
        assert!(html.contains("merhaba ğüşi"), "the saved HTML has the text");

        let png = renderer
            .render_png(
                &html,
                executed.card.width,
                executed.card.height,
                executed.card.scale,
            )
            .expect("the browser captured the saved HTML");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
        assert!(
            png.len() > 1000,
            "suspiciously small PNG: {} bytes",
            png.len()
        );

        println!("png: {} bytes; {}", png.len(), executed.human());
    }
}
