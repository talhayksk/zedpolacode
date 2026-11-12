//! Host-side output.
//!
//! Zed runs extensions inside a WASI sandbox that can only write to the
//! extension's own directory. Project files therefore have to be written by a
//! host process, so we hand the finished card to Node (which Zed ships with)
//! and let it write the files and drive a headless Chromium browser.

use zed_extension_api::{self as zed, process::Command, Worktree};

use polacode_core::render::Rendered;

use crate::options::Options;

const NODE_SCRIPT: &str = include_str!("zedpolacode.js");

/// Cards larger than this are not fun to pass through a command line.
const MAX_INLINE_HTML: usize = 20_000;

pub struct Outcome {
    pub display_png: Option<String>,
    pub display_html: Option<String>,
    pub warning: Option<String>,
}

pub fn emit(
    worktree: Option<&Worktree>,
    rendered: &Rendered,
    options: &Options,
    base: &str,
) -> Result<Outcome, String> {
    let root = worktree.map(|worktree| worktree.root_path());
    let (out_dir, use_tmp) = match (root.as_deref(), options.out_dir.as_deref()) {
        (Some(_), Some(relative)) if is_absolute(relative) => (relative.to_string(), false),
        (Some(root), Some(relative)) => (join_path(root, relative), false),
        (Some(root), None) => (join_path(root, ".zedpolacode"), false),
        (None, Some(absolute)) if is_absolute(absolute) => (absolute.to_string(), false),
        _ => (String::new(), true),
    };

    // The card has to exist on disk before the browser can open it. Writing to
    // the extension's own work dir is the one place the sandbox allows; `PWD`
    // gives us the matching host path (Zed sets it for us).
    let mut cleanup: Option<String> = None;
    let mut mode = "inline";
    let mut html_arg = rendered.html.clone();

    if let Some(work_dir) = host_work_dir() {
        let candidate = format!("{work_dir}/zedpolacode-card-{}.html", stamp());
        if std::fs::write(&candidate, &rendered.html).is_ok() {
            mode = "file";
            html_arg = candidate.clone();
            cleanup = Some(candidate);
        }
    }

    if mode == "inline" && rendered.html.len() > MAX_INLINE_HTML {
        return Err(
            "The card is too large to hand to the renderer. Try a smaller line range.".to_string(),
        );
    }

    let spec = format!(
        "{{\"mode\":{},\"html\":{},\"outDir\":{},\"base\":{},\"w\":{},\"h\":{},\"scale\":{},\"png\":{},\"wait\":1200,\"useTmp\":{}}}",
        quote(mode),
        quote(&html_arg),
        quote(&out_dir),
        quote(base),
        rendered.width,
        rendered.height,
        rendered.scale,
        options.format.wants_png(),
        use_tmp,
    );

    let result = run_node(worktree, &spec, options.format.wants_png(), root.as_deref());

    if let Some(path) = cleanup {
        let _ = std::fs::remove_file(path);
    }

    result
}

fn run_node(
    worktree: Option<&Worktree>,
    spec: &str,
    want_png: bool,
    root: Option<&str>,
) -> Result<Outcome, String> {
    let node = zed::node_binary_path()
        .ok()
        .or_else(|| worktree.and_then(|worktree| worktree.which("node")))
        .or_else(|| worktree.and_then(|worktree| worktree.which("nodejs")))
        .unwrap_or_else(|| "node".to_string());

    let mut command = Command::new(node.clone())
        .arg("-e")
        .arg(NODE_SCRIPT)
        .arg(spec);
    let output = command.output().map_err(|error| {
        format!(
            "Could not start Node (`{node}`): {error}. Zed ships its own Node runtime; if it is missing, install Node and put it on PATH."
        )
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let mut png: Option<String> = None;
    let mut html: Option<String> = None;
    let mut warnings: Vec<String> = Vec::new();
    let mut reported_ok = false;

    for line in stdout.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "ok" => reported_ok = true,
            "png" if !value.is_empty() => png = Some(value.to_string()),
            "html" if !value.is_empty() => html = Some(value.to_string()),
            "warn" if !value.is_empty() => warnings.push(value.to_string()),
            "error" if !value.is_empty() => {
                return Err(describe_error(value, &stderr));
            }
            _ => {}
        }
    }

    if !reported_ok && html.is_none() {
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(format!(
            "The Zed Polacode renderer did not answer. {}",
            clip(detail, 400)
        ));
    }

    if want_png && png.is_none() && warnings.is_empty() {
        warnings.push("png-missing".to_string());
    }

    let display_png = png.as_deref().map(|path| display_path(root, path));
    let display_html = html.as_deref().map(|path| display_path(root, path));

    Ok(Outcome {
        display_png,
        display_html,
        warning: if warnings.is_empty() {
            None
        } else {
            Some(warnings.join("; "))
        },
    })
}

fn describe_error(code: &str, stderr: &str) -> String {
    let message = match code {
        "missing-arguments" | "bad-arguments" => {
            "Zed Polacode could not pass the card to Node (internal error).".to_string()
        }
        "cannot-create-output-directory" => {
            "The output folder could not be created. Check `--out`.".to_string()
        }
        "cannot-write-html" => "The card could not be written to disk.".to_string(),
        other => format!("Zed Polacode renderer failed: {other}."),
    };
    if stderr.trim().is_empty() {
        message
    } else {
        format!("{message} {}", clip(stderr.trim(), 300))
    }
}

fn host_work_dir() -> Option<String> {
    // Zed sets PWD to the host path of the extension work dir (the same path it
    // preopens for us). The docs warn that `std::env::var` is not reliable in
    // extensions, so fall back to the current directory when it is missing.
    if let Ok(pwd) = std::env::var("PWD") {
        if let Some(dir) = usable_dir(&pwd) {
            return Some(dir);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(dir) = usable_dir(&cwd.to_string_lossy()) {
            return Some(dir);
        }
    }
    None
}

fn usable_dir(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed == "." || trimmed == "\\" {
        return None;
    }
    Some(trimmed.to_string())
}

pub(crate) fn is_absolute(path: &str) -> bool {
    if path.starts_with('/') || path.starts_with("\\\\") {
        return true;
    }
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'/' || bytes[2] == b'\\')
}

fn join_path(root: &str, relative: &str) -> String {
    format!(
        "{}/{}",
        root.trim_end_matches(['/', '\\']),
        relative.trim_start_matches(['/', '\\'])
    )
}

fn display_path(root: Option<&str>, full: &str) -> String {
    let normalized = full.replace('\\', "/");
    if let Some(root) = root {
        let root = root.replace('\\', "/");
        let root = root.trim_end_matches('/');
        if let Some(rest) = normalized.strip_prefix(root) {
            let rest = rest.trim_start_matches('/');
            if !rest.is_empty() {
                return rest.to_string();
            }
        }
    }
    normalized
}

fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut clipped: String = text.chars().take(limit).collect();
    clipped.push('…');
    clipped
}

fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_json_strings() {
        assert_eq!(quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(quote("line\nbreak"), "\"line\\nbreak\"");
    }

    #[test]
    fn detects_absolute_paths() {
        assert!(is_absolute("/tmp/x"));
        assert!(is_absolute("C:\\Users\\x"));
        assert!(is_absolute("D:/projeler"));
        assert!(!is_absolute(".zedpolacode"));
        assert!(!is_absolute("shot.png"));
    }

    #[test]
    fn rejects_unusable_work_dirs() {
        assert_eq!(usable_dir("  /tmp/work/  ").as_deref(), Some("/tmp/work"));
        assert_eq!(usable_dir("D:\\work\\").as_deref(), Some("D:\\work"));
        assert_eq!(usable_dir(""), None);
        assert_eq!(usable_dir("/"), None);
        assert_eq!(usable_dir("."), None);
    }

    #[test]
    fn joins_and_displays_paths() {
        assert_eq!(
            join_path("D:\\projeler\\app", ".zedpolacode"),
            "D:\\projeler\\app/.zedpolacode"
        );
        assert_eq!(
            display_path(
                Some("D:\\projeler\\app"),
                "D:\\projeler\\app/.zedpolacode/a.png"
            ),
            ".zedpolacode/a.png"
        );
        assert_eq!(display_path(None, "/tmp/a.png"), "/tmp/a.png");
    }
}
