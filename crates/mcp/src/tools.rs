//! The `snapshot` tool: schema, argument validation, card pipeline, result.
//!
//! Argument problems are JSON-RPC errors (`-32602`) because the caller can fix
//! them by sending a different request. Everything that fails after the request
//! was understood — an unreadable file, a range outside the file, a missing
//! browser — is a normal tool result with `isError: true`, because a model can
//! read that text and try something else.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};

use polacode_core::options::{valid_background, BG_PRESETS};
use polacode_core::{highlight, theme};

use crate::base64;
use crate::card::{self, Card, PngRenderer};
use crate::protocol::{ErrorObject, INVALID_PARAMS};

pub const TOOL_NAME: &str = "snapshot";

pub fn list() -> Value {
    json!({ "tools": [schema()] })
}

fn schema() -> Value {
    json!({
        "name": TOOL_NAME,
        "description": "Render code as a Polacode-style card and return it as a PNG image. Use it when the user asks for a screenshot, picture or shareable image of code, or wants a snippet turned into a card. Pass `path` to snapshot a file (optionally a `lines` range) or `code` for inline source. Producing the PNG needs Chrome, Edge, Chromium or Brave on this machine.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File to snapshot. Relative paths resolve against the server's working directory. Pass exactly one of `path` and `code`."
                },
                "code": {
                    "type": "string",
                    "description": "Inline source instead of a file. Pass exactly one of `path` and `code`."
                },
                "lines": {
                    "type": "string",
                    "description": "Line range for `path`: `40-70`, `40:70`, `40..70`, or `40-` for the rest of the file."
                },
                "lang": {
                    "type": "string",
                    "description": format!(
                        "Language id or file extension. Detected from `path` when omitted, otherwise `plaintext`. Known ids: {}.",
                        highlight::id_list()
                    )
                },
                "theme": {
                    "type": "string",
                    "description": format!(
                        "Color theme. Default `dracula`. Available: {}.",
                        theme::id_list()
                    )
                },
                "bg": {
                    "type": "string",
                    "description": format!(
                        "Background: one of {}, or a hex color such as `#1e1e2e`. Default `transparent`.",
                        BG_PRESETS.join(", ")
                    )
                },
                "title": {
                    "type": "string",
                    "description": "Title bar text. Defaults to the file name, or `snippet`."
                },
                "font_size": {
                    "type": "integer",
                    "minimum": 9,
                    "maximum": 40,
                    "description": "Font size in px. Default 15."
                },
                "padding": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 120,
                    "description": "Inner padding in px. Default 34."
                },
                "radius": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 48,
                    "description": "Corner radius in px. Default 14."
                },
                "line_numbers": {
                    "type": "boolean",
                    "description": "Show line numbers. Default true."
                },
                "window": {
                    "type": "boolean",
                    "description": "Show the title bar and window dots. Default true."
                },
                "shadow": {
                    "type": "boolean",
                    "description": "Draw the drop shadow. Default true."
                },
                "scale": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 4,
                    "description": "Device pixel ratio of the PNG. Default 2."
                },
                "out_dir": {
                    "type": "string",
                    "description": "Directory for the `.png` and `.html` files. Relative paths resolve against the server's working directory."
                },
                "write_files": {
                    "type": "boolean",
                    "description": "Write the `.png` and `.html` into `out_dir`. Requires `out_dir`."
                }
            },
            "required": [],
            "additionalProperties": false
        }
    })
}

/// The decoded and validated arguments of one `snapshot` call.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SnapshotArgs {
    pub path: Option<String>,
    pub code: Option<String>,
    pub lines: Option<String>,
    pub lang: Option<String>,
    pub theme: Option<String>,
    pub bg: Option<String>,
    pub title: Option<String>,
    pub font_size: Option<u32>,
    pub padding: Option<u32>,
    pub radius: Option<u32>,
    pub line_numbers: Option<bool>,
    pub window: Option<bool>,
    pub shadow: Option<bool>,
    pub scale: Option<u32>,
    pub out_dir: Option<String>,
    pub write_files: Option<bool>,
    /// `lines`, parsed once during validation.
    #[serde(skip)]
    pub lines_range: Option<(u32, u32)>,
}

impl SnapshotArgs {
    pub(crate) fn decode(value: &Value) -> Result<Self, ErrorObject> {
        let empty = Value::Object(serde_json::Map::new());
        let value = if value.is_null() { &empty } else { value };

        let mut args: Self = serde_json::from_value(value.clone())
            .map_err(|error| invalid_params(format!("Invalid arguments: {error}")))?;
        args.validate()?;
        Ok(args)
    }

    fn validate(&mut self) -> Result<(), ErrorObject> {
        match (&self.path, &self.code) {
            (Some(_), Some(_)) => {
                return Err(invalid_params("Pass either `path` or `code`, not both."))
            }
            (None, None) => {
                return Err(invalid_params(
                    "Pass `path` (a file) or `code` (inline source).",
                ))
            }
            _ => {}
        }

        if let Some(raw) = &self.lines {
            self.lines_range = Some(parse_lines(raw)?);
        }

        if let Some(name) = &self.theme {
            if theme::resolve(name).is_none() {
                return Err(invalid_params(format!(
                    "Unknown theme `{name}`. Available: {}.",
                    theme::id_list()
                )));
            }
        }

        if let Some(value) = &self.bg {
            if !valid_background(value) {
                return Err(invalid_params(format!(
                    "Unknown background `{value}`. Use one of: {}, or a hex color like `#1e1e2e`.",
                    BG_PRESETS.join(", ")
                )));
            }
        }

        check_range("font_size", self.font_size, 9, 40)?;
        check_range("padding", self.padding, 0, 120)?;
        check_range("radius", self.radius, 0, 48)?;
        check_range("scale", self.scale, 1, 4)?;

        if self.write_files == Some(true) && self.out_dir.is_none() {
            return Err(invalid_params("`write_files` requires `out_dir`."));
        }

        Ok(())
    }
}

/// `tools/call` entry point. `Err` is always a `-32602`: anything that can go
/// wrong later is reported inside a normal result.
pub fn call(params: &Value, renderer: &dyn PngRenderer) -> Result<Value, ErrorObject> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_params("`tools/call` needs a `name` string."))?;

    if name != TOOL_NAME {
        return Err(invalid_params(format!(
            "Unknown tool `{name}`. The only tool is `{TOOL_NAME}`."
        )));
    }

    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
    let args = SnapshotArgs::decode(&arguments)?;

    Ok(snapshot(&args, renderer))
}

fn snapshot(args: &SnapshotArgs, renderer: &dyn PngRenderer) -> Value {
    let card = match card::build(args) {
        Ok(card) => card,
        Err(message) => return tool_error(&message),
    };

    let png = match renderer.render_png(&card.html, card.width, card.height, card.scale) {
        Ok(png) => png,
        Err(error) => return tool_error(&error.message()),
    };

    let written = match (args.write_files, &args.out_dir) {
        (Some(true), Some(out_dir)) => match write_files(&card, &png, out_dir) {
            Ok(written) => written,
            Err(message) => return tool_error(&message),
        },
        _ => Written::default(),
    };

    json!({
        "content": [
            {
                "type": "image",
                "data": base64::encode(&png),
                "mimeType": "image/png"
            },
            {
                "type": "text",
                "text": summary(&card, &written)
            }
        ]
    })
}

#[derive(Debug, Default)]
pub(crate) struct Written {
    pub(crate) png: Option<PathBuf>,
    pub(crate) html: Option<PathBuf>,
}

/// Shared with the CLI: both surfaces write the same two files the same way.
pub(crate) fn write_files(card: &Card, png: &[u8], out_dir: &str) -> Result<Written, String> {
    std::fs::create_dir_all(out_dir)
        .map_err(|error| format!("Could not create `{out_dir}`: {error}"))?;

    let base = card.file_base();
    let png_path = Path::new(out_dir).join(format!("{base}.png"));
    let html_path = Path::new(out_dir).join(format!("{base}.html"));

    std::fs::write(&png_path, png)
        .map_err(|error| format!("Could not write `{}`: {error}", png_path.display()))?;
    std::fs::write(&html_path, &card.html)
        .map_err(|error| format!("Could not write `{}`: {error}", html_path.display()))?;

    Ok(Written {
        png: Some(png_path),
        html: Some(html_path),
    })
}

pub(crate) fn summary(card: &Card, written: &Written) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "lines: {}", card.lines);
    let _ = writeln!(text, "first_line: {}", card.first_line);
    let _ = writeln!(text, "language: {}", card.language);
    let _ = writeln!(text, "theme: {}", card.theme);
    let _ = writeln!(text, "title: {}", card.title);
    let _ = writeln!(text, "canvas_css: {}x{}", card.width, card.height);
    let _ = writeln!(
        text,
        "canvas_device: {}x{} @{}x",
        card.width.saturating_mul(card.scale),
        card.height.saturating_mul(card.scale),
        card.scale
    );
    if let Some(png) = &written.png {
        let _ = writeln!(text, "png: {}", png.display());
    }
    if let Some(html) = &written.html {
        let _ = writeln!(text, "html: {}", html.display());
    }

    while text.ends_with('\n') {
        text.pop();
    }
    text
}

/// The same field names as [`summary`], as one JSON object for the CLI's
/// `--json` flag. Keep the two in sync: a test in `cli.rs` asserts the key set.
pub(crate) fn summary_json(card: &Card, written: &Written) -> Value {
    let mut object = serde_json::Map::new();
    object.insert("lines".to_string(), json!(card.lines));
    object.insert("first_line".to_string(), json!(card.first_line));
    object.insert("language".to_string(), json!(card.language));
    object.insert("theme".to_string(), json!(card.theme));
    object.insert("title".to_string(), json!(card.title));
    object.insert(
        "canvas_css".to_string(),
        json!(format!("{}x{}", card.width, card.height)),
    );
    object.insert(
        "canvas_device".to_string(),
        json!(format!(
            "{}x{} @{}x",
            card.width.saturating_mul(card.scale),
            card.height.saturating_mul(card.scale),
            card.scale
        )),
    );
    if let Some(png) = &written.png {
        object.insert("png".to_string(), json!(png.display().to_string()));
    }
    if let Some(html) = &written.html {
        object.insert("html".to_string(), json!(html.display().to_string()));
    }
    Value::Object(object)
}

fn tool_error(message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true
    })
}

fn invalid_params(message: impl Into<String>) -> ErrorObject {
    ErrorObject::new(INVALID_PARAMS, message)
}

fn check_range(name: &str, value: Option<u32>, min: u32, max: u32) -> Result<(), ErrorObject> {
    if let Some(value) = value {
        if value < min || value > max {
            return Err(invalid_params(format!(
                "`{name}` must be between {min} and {max}, got {value}."
            )));
        }
    }
    Ok(())
}

/// The same syntax the slash command accepts: `40-70`, `40:70`, `40..70`,
/// `40+70`, `L40-L70`, a single line, or `40-` for "to the end of the file".
///
/// This mirrors `parse_lines` in the extension's `src/options.rs`, which words
/// its errors for a flag (`--lines`) instead of a tool argument. Keep the two in
/// sync; a third front end would be the point to move the parser into the core.
fn parse_lines(raw: &str) -> Result<(u32, u32), ErrorObject> {
    let cleaned = raw.trim().replace(['L', 'l'], "");
    let invalid = || invalid_params(format!("`lines` expects `start-end`, got `{raw}`."));

    let parts = cleaned
        .split_once("..")
        .or_else(|| cleaned.split_once('-'))
        .or_else(|| cleaned.split_once(':'))
        .or_else(|| cleaned.split_once('+'));

    match parts {
        Some((start, end)) => {
            let start = start.trim().parse::<u32>().map_err(|_| invalid())?;
            let end = if end.trim().is_empty() {
                0
            } else {
                end.trim().parse::<u32>().map_err(|_| invalid())?
            };
            if start == 0 {
                return Err(invalid());
            }
            if end != 0 && end < start {
                return Err(invalid_params(format!(
                    "Line range ends before it starts: `{raw}`."
                )));
            }
            Ok((start, end))
        }
        None => {
            let single = cleaned.trim().parse::<u32>().map_err(|_| invalid())?;
            if single == 0 {
                return Err(invalid());
            }
            Ok((single, single))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::TempDir;
    use crate::fixture::{snapshot_line, FakeRenderer, ONE_PIXEL_PNG, ONE_PIXEL_PNG_BASE64};

    fn call_arguments(arguments: Value) -> Result<Value, ErrorObject> {
        call(
            &json!({ "name": TOOL_NAME, "arguments": arguments }),
            &FakeRenderer,
        )
    }

    #[test]
    fn a_snippet_returns_an_image_block_and_a_summary() {
        let result = call_arguments(json!({ "code": "fn main() {}", "lang": "rust" })).expect("ok");
        let content = result["content"].as_array().expect("content");

        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["mimeType"], "image/png");
        assert_eq!(content[0]["data"], ONE_PIXEL_PNG_BASE64);
        assert_eq!(content[1]["type"], "text");

        let text = content[1]["text"].as_str().expect("text");
        assert!(text.contains("lines: 1"), "{text}");
        assert!(text.contains("language: rust"), "{text}");
        assert!(text.contains("theme: dracula"), "{text}");
        assert!(text.contains("canvas_css: "), "{text}");
        assert!(text.contains("canvas_device: "), "{text}");
        assert!(result.get("isError").is_none());
    }

    #[test]
    fn the_png_is_written_next_to_the_html_when_asked() {
        let dir = TempDir::create().expect("temp dir");
        let result = call_arguments(json!({
            "code": "fn main() {}",
            "lang": "rust",
            "title": "demo.rs",
            "write_files": true,
            "out_dir": dir.path.to_string_lossy(),
        }))
        .expect("ok");

        let png = std::fs::read(dir.path.join("demo.png")).expect("png written");
        assert_eq!(png, ONE_PIXEL_PNG);
        let html = std::fs::read_to_string(dir.path.join("demo.html")).expect("html written");
        assert!(html.starts_with("<!doctype html>"));

        let text = result["content"][1]["text"].as_str().expect("text");
        assert!(text.contains("demo.png"), "{text}");
        assert!(text.contains("demo.html"), "{text}");
    }

    #[test]
    fn a_missing_file_is_a_tool_error_not_a_protocol_error() {
        let result = call_arguments(json!({ "path": "definitely-not-here.rs" })).expect("result");
        assert_eq!(result["isError"], json!(true));
        let text = result["content"][0]["text"].as_str().expect("text");
        assert!(text.contains("Could not read"), "{text}");
    }

    #[test]
    fn a_failing_renderer_is_a_tool_error() {
        let result = call(
            &json!({ "name": TOOL_NAME, "arguments": { "code": "let x = 1;" } }),
            &crate::browser::BrowserRenderer::with_browser(None),
        )
        .expect("result");
        assert_eq!(result["isError"], json!(true));
        let text = result["content"][0]["text"].as_str().expect("text");
        assert!(text.contains("POLACODE_BROWSER"), "{text}");
    }

    #[test]
    fn an_unknown_tool_is_invalid_params() {
        let error = call(&json!({ "name": "nope" }), &FakeRenderer).unwrap_err();
        assert_eq!(error.code, INVALID_PARAMS);
        assert!(error.message.contains("nope"), "{}", error.message);
    }

    #[test]
    fn a_font_size_outside_the_range_is_invalid_params() {
        let error = call_arguments(json!({ "code": "x", "font_size": 41 })).unwrap_err();
        assert_eq!(error.code, INVALID_PARAMS);
        assert!(error.message.contains("font_size"), "{}", error.message);
    }

    #[test]
    fn an_unknown_background_is_invalid_params() {
        let error = call_arguments(json!({ "code": "x", "bg": "chartreuse" })).unwrap_err();
        assert_eq!(error.code, INVALID_PARAMS);
        assert!(error.message.contains("chartreuse"), "{}", error.message);
    }

    #[test]
    fn the_other_numeric_arguments_are_range_checked() {
        for arguments in [
            json!({ "code": "x", "padding": 121 }),
            json!({ "code": "x", "radius": 49 }),
            json!({ "code": "x", "scale": 5 }),
            json!({ "code": "x", "scale": 0 }),
        ] {
            assert_eq!(call_arguments(arguments).unwrap_err().code, INVALID_PARAMS);
        }
    }

    #[test]
    fn a_wrong_argument_type_is_invalid_params() {
        let error = call_arguments(json!({ "code": "x", "font_size": "big" })).unwrap_err();
        assert_eq!(error.code, INVALID_PARAMS);
    }

    #[test]
    fn an_unknown_argument_is_invalid_params() {
        let error = call_arguments(json!({ "code": "x", "colour": "red" })).unwrap_err();
        assert_eq!(error.code, INVALID_PARAMS);
        assert!(error.message.contains("colour"), "{}", error.message);
    }

    #[test]
    fn path_and_code_are_exclusive() {
        let error = call_arguments(json!({ "path": "a.rs", "code": "x" })).unwrap_err();
        assert_eq!(error.code, INVALID_PARAMS);
    }

    #[test]
    fn one_of_path_or_code_is_required() {
        let error = call_arguments(json!({})).unwrap_err();
        assert_eq!(error.code, INVALID_PARAMS);
    }

    #[test]
    fn a_reversed_line_range_is_invalid_params() {
        let error = call_arguments(json!({ "code": "x", "lines": "9-4" })).unwrap_err();
        assert_eq!(error.code, INVALID_PARAMS);
    }

    #[test]
    fn an_unknown_theme_is_invalid_params() {
        let error =
            call_arguments(json!({ "code": "x", "theme": "solarized-midnight" })).unwrap_err();
        assert_eq!(error.code, INVALID_PARAMS);
    }

    #[test]
    fn write_files_requires_out_dir() {
        let error = call_arguments(json!({ "code": "x", "write_files": true })).unwrap_err();
        assert_eq!(error.code, INVALID_PARAMS);
    }

    #[test]
    fn line_ranges_accept_every_documented_spelling() {
        assert_eq!(parse_lines("40-70").expect("range"), (40, 70));
        assert_eq!(parse_lines("40:70").expect("range"), (40, 70));
        assert_eq!(parse_lines("40..70").expect("range"), (40, 70));
        assert_eq!(parse_lines("40+70").expect("range"), (40, 70));
        assert_eq!(parse_lines("L40-L70").expect("range"), (40, 70));
        assert_eq!(parse_lines("40").expect("range"), (40, 40));
        assert_eq!(parse_lines("40-").expect("range"), (40, 0));
        assert!(parse_lines("0-2").is_err());
        assert!(parse_lines("x-y").is_err());
    }

    #[test]
    fn the_schema_describes_every_argument() {
        let schema = schema();
        assert_eq!(schema["name"], TOOL_NAME);
        assert_eq!(schema["inputSchema"]["type"], "object");
        assert_eq!(schema["inputSchema"]["required"], json!([]));
        assert_eq!(schema["inputSchema"]["additionalProperties"], json!(false));

        let properties = schema["inputSchema"]["properties"]
            .as_object()
            .expect("properties");
        for name in [
            "path",
            "code",
            "lines",
            "lang",
            "theme",
            "bg",
            "title",
            "font_size",
            "padding",
            "radius",
            "line_numbers",
            "window",
            "shadow",
            "scale",
            "out_dir",
            "write_files",
        ] {
            assert!(properties.contains_key(name), "missing `{name}`");
        }
    }

    #[test]
    fn a_tools_call_line_reaches_the_tool() {
        // Guards the fixture helper the server tests share.
        let line = snapshot_line(1, json!({ "code": "x" }));
        assert!(line.contains("\"snapshot\""));
    }
}
