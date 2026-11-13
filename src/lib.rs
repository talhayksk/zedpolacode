//! Zed Polacode.
//!
//! Zed extensions cannot open custom panels, so the snapshot feature is exposed
//! as a slash command: `/zedpolacode <file> --lines 12-40` renders a code card
//! (PNG + HTML) into `.zedpolacode/`.

mod host;
mod options;

use polacode_core::source::{dedent, normalize, slice_lines, SliceError};
use polacode_core::{highlight, render, theme};

use std::fmt::Write as _;

use zed_extension_api::{
    self as zed, SlashCommand, SlashCommandArgumentCompletion, SlashCommandOutput, Worktree,
};

struct ZedPolacodeExtension;

impl zed::Extension for ZedPolacodeExtension {
    fn new() -> Self {
        Self
    }

    fn run_slash_command(
        &self,
        command: SlashCommand,
        args: Vec<String>,
        worktree: Option<&Worktree>,
    ) -> zed::Result<SlashCommandOutput> {
        match command.name.as_str() {
            "/zedpolacode" => run(&args, worktree),
            other => Err(format!(
                "Zed Polacode does not handle the `{other}` command."
            )),
        }
    }

    fn complete_slash_command_argument(
        &self,
        _command: SlashCommand,
        args: Vec<String>,
    ) -> zed::Result<Vec<SlashCommandArgumentCompletion>> {
        Ok(complete(&args))
    }
}

zed::register_extension!(ZedPolacodeExtension);

struct Source {
    code: String,
    /// Path as typed by the user (or `snippet` for pasted code).
    label: String,
    first_line: u32,
    stem: String,
}

fn run(args: &[String], worktree: Option<&Worktree>) -> Result<SlashCommandOutput, String> {
    let options = options::Options::parse(args)?;

    if options.help {
        return Ok(text_output(options::help()));
    }

    let theme = theme::resolve(&options.theme).ok_or_else(|| {
        format!(
            "Unknown theme `{}`. Available: {}.",
            options.theme,
            theme::id_list()
        )
    })?;

    let source = resolve_source(&options, worktree)?;
    let code = dedent(&normalize(&source.code));
    if code.trim().is_empty() {
        return Err("Nothing to snapshot: the selection is empty.".to_string());
    }

    let language = highlight::language_for(options.lang.as_deref(), &source.label);
    let lines = highlight::tokenize(&code, language);
    if lines.is_empty() {
        return Err("Nothing to snapshot: the selection is empty.".to_string());
    }

    let title = options
        .title
        .clone()
        .unwrap_or_else(|| short_title(&source.label));

    let rendered = render::render(
        &lines,
        &options.card(),
        theme,
        &render::Meta {
            title: &title,
            first_line: source.first_line,
        },
    );

    let base = base_name(&source, &options);

    if rendered.width > 8000 || rendered.height > 12000 {
        return Err(format!(
            "The card would be {}×{} px, which is too large to render. Use a smaller line range or `--font-size 12`.",
            rendered.width * rendered.scale,
            rendered.height * rendered.scale
        ));
    }

    let outcome = host::emit(worktree, &rendered, &options, &base)?;

    Ok(text_output(summary(
        &rendered,
        &outcome,
        &options,
        language.id,
        theme.id,
        &title,
    )))
}

fn resolve_source(
    options: &options::Options,
    worktree: Option<&Worktree>,
) -> Result<Source, String> {
    if let Some(requested) = options.file.as_deref() {
        let worktree = worktree.ok_or_else(|| {
            "Reading a file needs an open project folder. Paste the code instead, or open the project first."
                .to_string()
        })?;

        let path = worktree_relative_path(worktree, requested);
        let raw = worktree
            .read_text_file(&path)
            .map_err(|error| format!("Could not read `{requested}`: {error}"))?;

        let (start, end) = options.lines.unwrap_or((1, 0));
        let slice = slice_lines(&raw, start, end).map_err(|error| match error {
            SliceError::Empty => format!("`{requested}` is empty."),
            SliceError::StartOutOfRange { start, total } => {
                format!("`{requested}` only has {total} lines, so line {start} does not exist.")
            }
            // `options::parse_lines` already rejects a reversed range, so this
            // branch only keeps the match total.
            SliceError::Reversed { start, end } => {
                format!("Line range ends before it starts: `{start}-{end}`.")
            }
        })?;

        return Ok(Source {
            stem: sanitize(&stem_of(requested)),
            label: requested.to_string(),
            first_line: slice.first_line,
            code: slice.code,
        });
    }

    if let Some(inline) = options.inline.clone() {
        let label = options
            .title
            .clone()
            .or_else(|| options.lang.clone())
            .unwrap_or_else(|| "snippet".to_string());
        return Ok(Source {
            stem: sanitize(
                &options
                    .title
                    .clone()
                    .unwrap_or_else(|| "snippet".to_string()),
            ),
            label,
            first_line: 1,
            code: inline,
        });
    }

    Err(
        "Nothing to snapshot. Give me a file and/or a line range, e.g. `/zedpolacode src/main.rs --lines 12-40`, or paste a code block. `/zedpolacode --help` lists every option."
            .to_string(),
    )
}

fn worktree_relative_path(worktree: &Worktree, requested: &str) -> String {
    let normalized = requested.replace('\\', "/");
    if !host::is_absolute(&normalized) {
        return normalized.trim_start_matches("./").to_string();
    }
    let root = worktree.root_path().replace('\\', "/");
    let root = root.trim_end_matches('/');
    match normalized.strip_prefix(root) {
        Some(rest) => rest.trim_start_matches('/').to_string(),
        None => normalized,
    }
}

fn stem_of(label: &str) -> String {
    let name = label
        .rsplit(|ch| ch == '/' || ch == '\\')
        .next()
        .unwrap_or(label);
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => name.to_string(),
    }
}

fn short_title(label: &str) -> String {
    label
        .rsplit(|ch| ch == '/' || ch == '\\')
        .next()
        .unwrap_or(label)
        .to_string()
}

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

fn base_name(source: &Source, options: &options::Options) -> String {
    let range = match options.lines {
        Some((start, end)) if end == 0 => format!("-L{start}-end"),
        Some((start, end)) if start == end => format!("-L{start}"),
        Some((start, end)) => format!("-L{start}-{end}"),
        None => String::new(),
    };
    format!("{}{}", source.stem, range)
}

fn summary(
    rendered: &render::Rendered,
    outcome: &host::Outcome,
    options: &options::Options,
    language: &str,
    theme: &str,
    title: &str,
) -> String {
    let mut text = String::new();
    let _ = writeln!(
        text,
        "**Zed Polacode** · {} lines · `{}` · `{}`",
        rendered.lines, language, theme
    );
    let _ = writeln!(text, "Title: `{}`\n", title);

    let mut files = Vec::new();
    if options.format.wants_png() {
        match &outcome.display_png {
            Some(png) => files.push(format!(
                "- PNG `{png}` — {}×{} px (@{}x)",
                rendered.width * rendered.scale,
                rendered.height * rendered.scale,
                rendered.scale
            )),
            None => files.push("- PNG not produced".to_string()),
        }
    }
    if options.format.wants_html() || outcome.display_png.is_none() {
        if let Some(html) = &outcome.display_html {
            files.push(format!("- HTML `{html}`"));
        }
    }
    text.push_str(&files.join("\n"));
    text.push('\n');

    if let Some(warning) = &outcome.warning {
        if warning.contains("no-browser") {
            let _ = writeln!(
                text,
                "\n> No PNG: Chrome, Edge, Chromium and Brave were all missing. The HTML card is ready — open it in a browser to grab a screenshot, or point `ZEDPOLACODE_BROWSER` at a browser binary."
            );
        } else if warning.contains("png-missing") {
            let _ = writeln!(
                text,
                "\n> No PNG was produced; the HTML card was written. Try again, or use `--format html`."
            );
        } else {
            let _ = writeln!(text, "\n> Warning: {warning}");
        }
    }

    let _ = writeln!(
        text,
        "\nTry other looks: `--theme nord`, `--bg gradient`, `--font-size 17`."
    );

    text
}

fn text_output(text: String) -> SlashCommandOutput {
    SlashCommandOutput {
        text,
        sections: Vec::new(),
    }
}

const FLAGS: &[(&str, &str)] = &[
    ("--lines", "line range, e.g. 12-40"),
    ("--theme", "color theme"),
    ("--bg", "background"),
    ("--lang", "force the language"),
    ("--title", "title text"),
    ("--font-size", "font size"),
    ("--padding", "inner padding"),
    ("--radius", "corner radius"),
    ("--format", "png | html | both"),
    ("--out", "output folder"),
    ("--no-line-numbers", "hide line numbers"),
    ("--no-window", "hide the title bar"),
    ("--no-shadow", "turn the shadow off"),
    ("--help", "all options"),
];

fn complete(args: &[String]) -> Vec<SlashCommandArgumentCompletion> {
    let current = args.last().cloned().unwrap_or_default();
    let previous = if args.len() >= 2 {
        args[args.len() - 2].clone()
    } else {
        String::new()
    };

    if current.starts_with("--") && !current.contains('=') {
        return FLAGS
            .iter()
            .filter(|(flag, _)| flag.starts_with(current.as_str()))
            .map(|(flag, description)| SlashCommandArgumentCompletion {
                label: format!("{flag} — {description}"),
                new_text: (*flag).to_string(),
                run_command: false,
            })
            .collect();
    }

    let (flag, prefix, with_equals) = if current.starts_with("--") && current.contains('=') {
        let (name, value) = current.split_once('=').unwrap_or((current.as_str(), ""));
        (name.to_string(), value.to_string(), true)
    } else if previous.starts_with("--") {
        (previous.clone(), current.clone(), false)
    } else {
        return Vec::new();
    };

    let candidates: Vec<String> = match flag.as_str() {
        "--theme" => theme::ids().into_iter().map(String::from).collect(),
        "--bg" | "--background" => options::BG_PRESETS
            .iter()
            .map(|preset| preset.to_string())
            .collect(),
        "--lang" | "--language" => highlight::LANGUAGES
            .iter()
            .map(|language| language.id.to_string())
            .collect(),
        "--format" => vec!["png".to_string(), "html".to_string(), "both".to_string()],
        _ => Vec::new(),
    };

    candidates
        .into_iter()
        .filter(|candidate| candidate.starts_with(prefix.as_str()))
        .map(|candidate| SlashCommandArgumentCompletion {
            label: candidate.clone(),
            new_text: if with_equals {
                format!("{flag}={candidate}")
            } else {
                candidate
            },
            run_command: false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_after_the_source_and_range() {
        let source = Source {
            code: String::new(),
            label: "src/store.rs".to_string(),
            first_line: 10,
            stem: sanitize(&stem_of("src/store.rs")),
        };
        let mut options = options::Options::default();
        assert_eq!(base_name(&source, &options), "store");
        options.lines = Some((10, 24));
        assert_eq!(base_name(&source, &options), "store-L10-24");
        options.lines = Some((10, 10));
        assert_eq!(base_name(&source, &options), "store-L10");
    }

    #[test]
    fn sanitizes_file_stems() {
        assert_eq!(sanitize("my file.rs"), "my-file.rs");
        assert_eq!(sanitize(""), "snippet");
    }

    #[test]
    fn completes_flags_and_values() {
        let flags = complete(&[String::from("--the")]);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].new_text, "--theme");

        let themes = complete(&[String::from("--theme"), String::from("dra")]);
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].new_text, "dracula");

        let equals = complete(&[String::from("--theme=gi")]);
        assert_eq!(equals[0].new_text, "--theme=github-dark");
    }

    /// Not a real test: writes a card to a temp file so the Node/browser path
    /// can be exercised by hand
    /// (`cargo test write_preview_artifact -- --ignored --nocapture`).
    #[test]
    #[ignore = "writes a preview artifact for manual inspection"]
    fn write_preview_artifact() {
        let code = "use std::collections::HashMap;\n\n/// Builds the greeting people see on the splash screen.\npub fn greet(name: &str) -> String {\n    let mut counts: HashMap<String, u32> = HashMap::new();\n    *counts.entry(name.to_string()).or_insert(0) += 1;\n    format!(\"merhaba {name} — {} kez\", counts[name])\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn greets() {\n        assert_eq!(super::greet(\"zed\"), \"merhaba zed — 1 kez\");\n    }\n}\n";
        let mut args = vec!["--theme".to_string(), "dracula".to_string()];
        args.extend(
            std::env::var("ZEDPOLACODE_PREVIEW_ARGS")
                .unwrap_or_default()
                .split_whitespace()
                .map(str::to_string),
        );
        let options = options::Options::parse(&args).unwrap();
        let theme = theme::resolve(&options.theme).unwrap();
        let lines = highlight::tokenize(code, highlight::language_for(Some("rust"), "snippet"));
        let rendered = render::render(
            &lines,
            &options.card(),
            theme,
            &render::Meta {
                title: "zedpolacode/src/lib.rs",
                first_line: 1,
            },
        );
        let path = std::env::temp_dir().join("zedpolacode-preview.html");
        std::fs::write(&path, &rendered.html).unwrap();
        println!("HTML={}", path.display());
        println!(
            "W={} H={} SCALE={}",
            rendered.width, rendered.height, rendered.scale
        );
    }
}
