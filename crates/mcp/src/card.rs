//! From tool arguments to a rendered card, and from a card to PNG bytes.
//!
//! The pure part (`build`) reuses `polacode-core` for highlighting, theming,
//! HTML and canvas measurement, exactly like the Zed extension does. The impure
//! part is behind [`PngRenderer`] so tests can run the whole pipeline with a
//! fixed PNG and no browser.

use polacode_core::options::CardOptions;
use polacode_core::source::{self, SliceError};
use polacode_core::{highlight, render, theme};

use crate::tools::SnapshotArgs;

/// Turns card HTML into PNG bytes.
pub trait PngRenderer {
    fn render_png(
        &self,
        html: &str,
        width: u32,
        height: u32,
        scale: u32,
    ) -> Result<Vec<u8>, RenderError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// No Chromium-based browser was found on this machine.
    NoBrowser,
    /// A browser was found but the screenshot failed.
    Failed(String),
}

/// One sentence, shared by both failure messages, that names the knobs a user
/// would set and what still works without a browser.
const BROWSER_HINT: &str = "Set `POLACODE_BROWSER` (or `CHROME_PATH`) to a Chrome, Edge, Chromium or Brave executable; rendering the PNG is the only step that needs one.";

impl RenderError {
    /// A message written for the agent that called the tool.
    pub fn message(&self) -> String {
        match self {
            RenderError::NoBrowser => format!("No Chromium-based browser found. {BROWSER_HINT}"),
            RenderError::Failed(detail) => {
                format!("The browser did not produce a PNG: {detail}. {BROWSER_HINT}")
            }
        }
    }
}

/// A card that is ready to be turned into a PNG.
#[derive(Debug)]
pub struct Card {
    pub html: String,
    /// Canvas size in CSS pixels.
    pub width: u32,
    pub height: u32,
    /// Device pixel ratio to capture at.
    pub scale: u32,
    pub lines: usize,
    pub first_line: u32,
    pub language: &'static str,
    pub theme: &'static str,
    pub title: String,
    /// File name stem, e.g. `store`.
    pub stem: String,
    /// The range the user asked for, as typed (`end == 0` means "to the end").
    pub range: Option<(u32, u32)>,
}

impl Card {
    /// File name without extension, e.g. `store-L10-24`.
    pub fn file_base(&self) -> String {
        let range = match self.range {
            Some((start, 0)) => format!("-L{start}-end"),
            Some((start, end)) if start == end => format!("-L{start}"),
            Some((start, end)) => format!("-L{start}-{end}"),
            None => String::new(),
        };
        format!("{}{range}", self.stem)
    }
}

pub fn build(args: &SnapshotArgs) -> Result<Card, String> {
    let (selected, first_line) = read_source(args)?;
    let label = label_of(args);

    let code = source::dedent(&source::normalize(&selected));
    if code.trim().is_empty() {
        return Err("Nothing to snapshot: the selection is empty.".to_string());
    }

    let language = highlight::language_for(args.lang.as_deref(), &label);
    let lines = highlight::tokenize(&code, language);
    if lines.is_empty() {
        return Err("Nothing to snapshot: the selection is empty.".to_string());
    }

    let theme = theme::resolve(args.theme.as_deref().unwrap_or_default())
        .unwrap_or_else(theme::default_theme);
    let title = args.title.clone().unwrap_or_else(|| short_title(&label));

    let rendered = render::render(
        &lines,
        &card_options(args),
        theme,
        &render::Meta {
            title: &title,
            first_line,
        },
    );

    let scale = args.scale.unwrap_or(rendered.scale);
    // `saturating_mul`, not `*`: a pathological input must end up in the
    // "too large" error below, not in an overflow panic in debug builds.
    let device_width = rendered.width.saturating_mul(scale);
    let device_height = rendered.height.saturating_mul(scale);
    if device_width > 8000 || device_height > 12000 {
        return Err(format!(
            "The card would be {device_width}×{device_height} device px, which is too large to render. Use a smaller line range or a smaller `font_size`."
        ));
    }

    Ok(Card {
        html: rendered.html,
        width: rendered.width,
        height: rendered.height,
        scale,
        lines: rendered.lines,
        first_line,
        language: language.id,
        theme: theme.id,
        title,
        stem: sanitize(&stem_of(&label)),
        // `lines` only selects rows of a file; inline code keeps all of it.
        range: if args.path.is_some() {
            args.lines_range
        } else {
            None
        },
    })
}

/// The source text and the 1-based number its first line carries.
fn read_source(args: &SnapshotArgs) -> Result<(String, u32), String> {
    if let Some(path) = &args.path {
        let raw = std::fs::read_to_string(path)
            .map_err(|error| format!("Could not read `{path}`: {error}"))?;
        // `lines` only means something for a file; inline code is taken as-is,
        // like the slash command does.
        let (start, end) = args.lines_range.unwrap_or((1, 0));
        let slice = source::slice_lines(&raw, start, end).map_err(|error| describe(path, error))?;
        return Ok((slice.code, slice.first_line));
    }

    match &args.code {
        Some(code) => Ok((code.clone(), 1)),
        // `SnapshotArgs::decode` rejects this, so this branch is defensive.
        None => Err("Nothing to snapshot: pass `path` or `code`.".to_string()),
    }
}

fn describe(label: &str, error: SliceError) -> String {
    match error {
        SliceError::Empty => format!("`{label}` is empty."),
        SliceError::StartOutOfRange { start, total } => {
            format!("`{label}` only has {total} lines, so line {start} does not exist.")
        }
        // The line range is validated before the card is built, so this is
        // defensive too.
        SliceError::Reversed { start, end } => {
            format!("Line range ends before it starts: `{start}-{end}`.")
        }
    }
}

/// What the source calls itself: the path, or the title/language of inline code.
fn label_of(args: &SnapshotArgs) -> String {
    if let Some(path) = &args.path {
        return path.clone();
    }
    args.title
        .clone()
        .or_else(|| args.lang.clone())
        .unwrap_or_else(|| "snippet".to_string())
}

fn short_title(label: &str) -> String {
    label
        .rsplit(|ch| ch == '/' || ch == '\\')
        .next()
        .unwrap_or(label)
        .to_string()
}

fn stem_of(label: &str) -> String {
    let name = short_title(label);
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => name,
    }
}

/// Keeps the file name safe across platforms and file systems.
fn sanitize(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "snippet".to_string()
    } else {
        trimmed.chars().take(60).collect()
    }
}

/// `CardOptions` for a set of arguments; anything absent keeps the core default.
pub fn card_options(args: &SnapshotArgs) -> CardOptions {
    let mut options = CardOptions::default();
    if let Some(font_size) = args.font_size {
        options.font_size = font_size;
    }
    if let Some(padding) = args.padding {
        options.padding = padding;
    }
    if let Some(radius) = args.radius {
        options.radius = radius;
    }
    if let Some(line_numbers) = args.line_numbers {
        options.line_numbers = line_numbers;
    }
    if let Some(window) = args.window {
        options.window = window;
    }
    if let Some(shadow) = args.shadow {
        options.shadow = shadow;
    }
    if let Some(bg) = &args.bg {
        options.bg = bg.clone();
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::TempDir;
    use crate::tools::SnapshotArgs;
    use serde_json::json;

    fn args(value: serde_json::Value) -> SnapshotArgs {
        SnapshotArgs::decode(&value).expect("valid arguments")
    }

    #[test]
    fn inline_code_keeps_every_line_and_starts_at_one() {
        let card = build(&args(json!({
            "code": "    fn main() {\n        run();\n    }",
            "lang": "rust",
            "title": "main.rs",
        })))
        .expect("card");
        assert_eq!(card.lines, 3);
        assert_eq!(card.first_line, 1);
        assert_eq!(card.language, "rust");
        assert_eq!(card.theme, "dracula");
        assert_eq!(card.title, "main.rs");
        assert_eq!(card.stem, "main");
        assert_eq!(card.file_base(), "main");
        assert!(card.html.starts_with("<!doctype html>"));
    }

    #[test]
    fn a_file_range_slices_and_names_the_card() {
        let dir = TempDir::create().expect("temp dir");
        let path = dir.path.join("store.rs");
        std::fs::write(&path, "one\ntwo\nthree\nfour\n").expect("write");

        let card = build(&args(json!({
            "path": path.to_string_lossy(),
            "lines": "2-3",
        })))
        .expect("card");
        assert_eq!(card.lines, 2);
        assert_eq!(card.first_line, 2);
        assert_eq!(card.language, "rust");
        assert_eq!(card.title, "store.rs");
        assert_eq!(card.file_base(), "store-L2-3");
    }

    #[test]
    fn an_open_ended_range_runs_to_the_end_of_the_file() {
        let dir = TempDir::create().expect("temp dir");
        let path = dir.path.join("notes.txt");
        std::fs::write(&path, "one\ntwo\nthree\n").expect("write");

        let card = build(&args(json!({
            "path": path.to_string_lossy(),
            "lines": "2-",
        })))
        .expect("card");
        assert_eq!(card.lines, 2);
        assert_eq!(card.first_line, 2);
        assert_eq!(card.language, "plaintext");
        assert_eq!(card.file_base(), "notes-L2-end");
    }

    #[test]
    fn lines_are_ignored_for_inline_code() {
        // `lines` selects rows of a file; inline source is used as given.
        let card = build(&args(json!({ "code": "one\ntwo", "lines": "2-2" }))).expect("card");
        assert_eq!(card.lines, 2);
        assert_eq!(card.first_line, 1);
        assert_eq!(card.file_base(), "snippet");
    }

    #[test]
    fn a_missing_file_is_a_runtime_error() {
        let message = build(&args(json!({ "path": "definitely-not-here.rs" }))).unwrap_err();
        assert!(message.contains("Could not read"), "{message}");
    }

    #[test]
    fn a_range_past_the_file_is_a_runtime_error() {
        let dir = TempDir::create().expect("temp dir");
        let path = dir.path.join("short.txt");
        std::fs::write(&path, "one\n").expect("write");

        let message = build(&args(json!({
            "path": path.to_string_lossy(),
            "lines": "5-6",
        })))
        .unwrap_err();
        assert!(message.contains("only has 1 lines"), "{message}");
    }

    #[test]
    fn the_scale_argument_overrides_the_capture_ratio() {
        let card = build(&args(json!({ "code": "let x = 1;", "scale": 3 }))).expect("card");
        assert_eq!(card.scale, 3);
    }

    #[test]
    fn a_huge_card_is_rejected_before_the_browser_sees_it() {
        let code = "line\n".repeat(400);
        let message = build(&args(json!({ "code": code, "font_size": 40 }))).unwrap_err();
        assert!(message.contains("too large"), "{message}");
    }

    #[test]
    fn a_render_failure_names_the_environment_knobs() {
        for message in [
            RenderError::NoBrowser.message(),
            RenderError::Failed("could not start `x`".to_string()).message(),
        ] {
            assert!(message.contains("POLACODE_BROWSER"), "{message}");
            assert!(message.contains("CHROME_PATH"), "{message}");
            assert!(
                message.contains("Chrome, Edge, Chromium or Brave"),
                "{message}"
            );
        }
    }

    #[test]
    fn sanitizes_file_stems() {
        assert_eq!(sanitize("my file.rs"), "my-file.rs");
        assert_eq!(sanitize(""), "snippet");
        assert_eq!(stem_of("src/store.rs"), "store");
        assert_eq!(stem_of("snippet"), "snippet");
    }
}
