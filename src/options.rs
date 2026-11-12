//! Command line parsing for `/polacode`.

use crate::highlight;
use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Html,
    Both,
}

impl Format {
    pub fn wants_png(self) -> bool {
        matches!(self, Format::Png | Format::Both)
    }

    pub fn wants_html(self) -> bool {
        matches!(self, Format::Html | Format::Both)
    }
}

pub const BG_PRESETS: &[&str] = &[
    "transparent",
    "dark",
    "light",
    "gradient",
    "sunset",
    "ocean",
    "aurora",
    "midnight",
    "mono",
];

#[derive(Debug, Clone)]
pub struct Options {
    pub file: Option<String>,
    pub inline: Option<String>,
    pub lines: Option<(u32, u32)>,
    pub lang: Option<String>,
    pub theme: String,
    pub title: Option<String>,
    pub line_numbers: bool,
    pub window: bool,
    pub shadow: bool,
    pub bg: String,
    pub font_size: u32,
    pub padding: u32,
    pub radius: u32,
    pub out_dir: Option<String>,
    pub format: Format,
    pub help: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            file: None,
            inline: None,
            lines: None,
            lang: None,
            theme: theme::default_theme().id.to_string(),
            title: None,
            line_numbers: true,
            window: true,
            shadow: true,
            bg: "transparent".to_string(),
            font_size: 15,
            padding: 34,
            radius: 14,
            out_dir: None,
            format: Format::Png,
            help: false,
        }
    }
}

const KNOWN_EXTENSIONS: &[&str] = &[
    "rs", "py", "pyw", "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "go", "c", "h", "cc",
    "cpp", "cxx", "hpp", "hh", "cs", "java", "kt", "kts", "swift", "rb", "php", "sh", "bash",
    "zsh", "fish", "ps1", "sql", "lua", "zig", "html", "htm", "xml", "svg", "vue", "svelte", "css",
    "scss", "sass", "less", "json", "jsonc", "yml", "yaml", "toml", "ini", "conf", "cfg", "env",
    "md", "txt", "log", "lock", "dart", "ex", "exs", "erl", "hs", "ml", "clj", "r", "jl", "nim",
    "v", "sol", "proto", "graphql", "gql", "tf",
];

fn looks_like_path(value: &str) -> bool {
    if value.is_empty() || value.contains('\n') || value.contains('(') || value.contains('=') {
        return false;
    }
    if value.contains('/') || value.contains('\\') || value.starts_with('.') {
        return true;
    }
    match value.rsplit_once('.') {
        Some((stem, ext)) => {
            !stem.is_empty() && KNOWN_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
        }
        None => false,
    }
}

fn take_value(
    inline: Option<String>,
    args: &[String],
    index: &mut usize,
    flag: &str,
) -> Result<String, String> {
    if let Some(value) = inline {
        return Ok(value);
    }
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("`{flag}` needs a value. See `/polacode --help`."))
}

fn parse_number(flag: &str, raw: &str, min: u32, max: u32) -> Result<u32, String> {
    let value = raw
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("`{flag}` expects a number, got `{raw}`."))?;
    if value < min || value > max {
        return Err(format!("`{flag}` must be between {min} and {max}."));
    }
    Ok(value)
}

/// Accepts `12-30`, `12:30`, `12..30`, `12+30`, `L12-L30` or a single line.
fn parse_lines(raw: &str) -> Result<(u32, u32), String> {
    let cleaned = raw.trim().replace(['L', 'l'], "");
    let invalid = || format!("`--lines` expects `start-end`, got `{raw}`.");

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
                return Err(format!("Line range ends before it starts: `{raw}`."));
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

fn valid_background(value: &str) -> bool {
    if BG_PRESETS.contains(&value) {
        return true;
    }
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    }
    false
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Options::default();
        let mut positional: Vec<String> = Vec::new();
        let mut index = 0usize;

        while index < args.len() {
            let arg = args[index].clone();

            if arg == "--" {
                index += 1;
                while index < args.len() {
                    positional.push(args[index].clone());
                    index += 1;
                }
                break;
            }

            if arg == "-h" || arg == "--help" {
                options.help = true;
                index += 1;
                continue;
            }

            if !arg.starts_with("--") {
                positional.push(arg);
                index += 1;
                continue;
            }

            let (flag, inline) = match arg.split_once('=') {
                Some((name, value)) => (name.to_string(), Some(value.to_string())),
                None => (arg.clone(), None),
            };

            match flag.as_str() {
                "--theme" => options.theme = take_value(inline, args, &mut index, "--theme")?,
                "--lang" | "--language" => {
                    options.lang = Some(take_value(inline, args, &mut index, "--lang")?)
                }
                "--title" => options.title = Some(take_value(inline, args, &mut index, "--title")?),
                "--file" => options.file = Some(take_value(inline, args, &mut index, "--file")?),
                "--lines" | "--line" => {
                    let raw = take_value(inline, args, &mut index, "--lines")?;
                    options.lines = Some(parse_lines(&raw)?);
                }
                "--bg" | "--background" => {
                    let raw = take_value(inline, args, &mut index, "--bg")?;
                    if !valid_background(&raw) {
                        return Err(format!(
                            "Unknown background `{raw}`. Use one of: {}, or a hex color like `#1e1e2e`.",
                            BG_PRESETS.join(", ")
                        ));
                    }
                    options.bg = raw;
                }
                "--font-size" => {
                    let raw = take_value(inline, args, &mut index, "--font-size")?;
                    options.font_size = parse_number("--font-size", &raw, 9, 40)?;
                }
                "--padding" => {
                    let raw = take_value(inline, args, &mut index, "--padding")?;
                    options.padding = parse_number("--padding", &raw, 0, 120)?;
                }
                "--radius" => {
                    let raw = take_value(inline, args, &mut index, "--radius")?;
                    options.radius = parse_number("--radius", &raw, 0, 48)?;
                }
                "--out" | "--out-dir" => {
                    options.out_dir = Some(take_value(inline, args, &mut index, "--out")?)
                }
                "--format" => {
                    let raw = take_value(inline, args, &mut index, "--format")?;
                    options.format = match raw.trim().to_ascii_lowercase().as_str() {
                        "png" | "image" => Format::Png,
                        "html" => Format::Html,
                        "both" | "all" => Format::Both,
                        other => {
                            return Err(format!(
                                "`--format` expects `png`, `html` or `both`, got `{other}`."
                            ))
                        }
                    };
                }
                "--line-numbers" => options.line_numbers = true,
                "--no-line-numbers" | "--no-numbers" => options.line_numbers = false,
                "--window" => options.window = true,
                "--no-window" | "--plain" => options.window = false,
                "--shadow" => options.shadow = true,
                "--no-shadow" => options.shadow = false,
                "--numbers" => options.line_numbers = true,
                other => {
                    return Err(format!(
                        "Unknown option `{other}`. Run `/polacode --help` to see what is available."
                    ))
                }
            }

            index += 1;
        }

        if options.help {
            return Ok(options);
        }

        // A fenced block pasted into the assistant arrives as ```` ```rust ```` tokens.
        let mut fenced_language: Option<String> = None;
        if let Some(first) = positional.first().cloned() {
            if let Some(rest) = first.strip_prefix("```") {
                let lang = rest.trim().trim_end_matches('`').to_string();
                if !lang.is_empty() {
                    fenced_language = Some(lang);
                }
                positional.remove(0);
            }
        }
        if let Some(last) = positional.last().cloned() {
            let trimmed = last.trim_end();
            if trimmed == "```" {
                positional.pop();
            } else if let Some(stripped) = trimmed.strip_suffix("```") {
                positional.pop();
                if !stripped.is_empty() {
                    positional.push(stripped.to_string());
                }
            }
        }

        if options.file.is_none() {
            if let Some(first) = positional.first().cloned() {
                if looks_like_path(&first) {
                    options.file = Some(first);
                    positional.remove(0);
                }
            }
        }

        if !positional.is_empty() {
            let code = positional.join(" ");
            match options.inline {
                Some(existing) => {
                    return Err(format!(
                        "Two code snippets were given (`{}` and `{}`). Pass a single block.",
                        short(&existing),
                        short(&code)
                    ))
                }
                None => options.inline = Some(code),
            }
        }

        if options.lang.is_none() {
            options.lang = fenced_language;
        }

        Ok(options)
    }
}

fn short(value: &str) -> String {
    let mut trimmed: String = value.chars().take(24).collect();
    if value.chars().count() > 24 {
        trimmed.push('…');
    }
    trimmed
}

pub fn help() -> String {
    format!(
        "\
**Zed Polacode** — turn code into a shareable image.

```text
/polacode <file> [--lines 12-40] [options]
/polacode [options] ```lang <code> ```
```

**Examples**

- `/polacode src/main.rs --lines 40-70` → lines 40-70 of the file
- `/polacode src/store.rs` → the whole file
- `/polacode --lines 12-30 --theme nord --bg gradient src/app.tsx`

**Options**

| Option | Description |
| --- | --- |
| `--lines <a-b>` | Line range (`40-70`, `40:70`, `40..70`, `40-` for \"to the end\") |
| `--lang <id>` | Force the language (`rust`, `py`, `ts`, ...) |
| `--theme <id>` | Color theme: {} |
| `--bg <name\\|#hex>` | Background: {} |
| `--title <text>` | Text shown in the title bar |
| `--font-size <px>` | Font size (default 15) |
| `--padding <px>` | Inner padding (default 34) |
| `--radius <px>` | Corner radius (default 14) |
| `--no-line-numbers` | Hide line numbers |
| `--no-window` | Hide the title bar and window dots |
| `--no-shadow` | Turn the drop shadow off |
| `--out <dir>` | Output folder (default `.polacode`) |
| `--format <png\\|html\\|both>` | Output format (default `png`) |

**Languages:** {}

Rendering a PNG needs Chrome, Edge, Chromium or Brave on your system. Without one the
HTML card is still written — open it in a browser and take a screenshot. Point
`ZEDPOLACODE_BROWSER` at a browser binary to pick one explicitly.",
        theme::id_list(),
        BG_PRESETS.join(", "),
        highlight::id_list()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn parses_file_and_range() {
        let options = Options::parse(&args(&["src/main.rs", "--lines", "12-30"])).unwrap();
        assert_eq!(options.file.as_deref(), Some("src/main.rs"));
        assert_eq!(options.lines, Some((12, 30)));
        assert!(options.inline.is_none());
    }

    #[test]
    fn parses_inline_code() {
        let options = Options::parse(&args(&["fn", "main()", "{}"])).unwrap();
        assert!(options.file.is_none());
        assert_eq!(options.inline.as_deref(), Some("fn main() {}"));
    }

    #[test]
    fn strips_fence_and_picks_language() {
        let options = Options::parse(&args(&["```rust", "let", "x", "=", "1;", "```"])).unwrap();
        assert_eq!(options.lang.as_deref(), Some("rust"));
        assert_eq!(options.inline.as_deref(), Some("let x = 1;"));
    }

    #[test]
    fn inline_values_and_flags() {
        let options = Options::parse(&args(&[
            "--theme=nord",
            "--bg=#101010",
            "--no-line-numbers",
        ]))
        .unwrap();
        assert_eq!(options.theme, "nord");
        assert_eq!(options.bg, "#101010");
        assert!(!options.line_numbers);
    }

    #[test]
    fn open_ended_range() {
        let options = Options::parse(&args(&["--lines", "40-"])).unwrap();
        assert_eq!(options.lines, Some((40, 0)));
    }

    #[test]
    fn rejects_bad_values() {
        assert!(Options::parse(&args(&["--bg", "chartreuse"])).is_err());
        assert!(Options::parse(&args(&["--lines", "9-4"])).is_err());
        assert!(Options::parse(&args(&["--font-size", "2"])).is_err());
        assert!(Options::parse(&args(&["--wat"])).is_err());
    }

    #[test]
    fn path_sniffing() {
        assert!(looks_like_path("src/lib.rs"));
        assert!(looks_like_path("lib.rs"));
        assert!(!looks_like_path("vec![1,2]"));
        assert!(!looks_like_path("x = 1"));
    }
}
