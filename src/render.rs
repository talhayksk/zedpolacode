//! Turns highlighted lines into a self-contained HTML card and tells the host
//! renderer how big the picture should be.

use std::fmt::Write as _;

use crate::highlight::{Kind, Token};
use crate::options::Options;
use crate::theme::Theme;

const MONO: &str = "\"JetBrains Mono\",\"Cascadia Mono\",\"Cascadia Code\",\"Fira Code\",Menlo,Consolas,\"DejaVu Sans Mono\",monospace";

/// Rough advance width of a monospace glyph relative to the font size.
/// Slightly generous so the measured canvas never clips the longest line.
const CHAR_RATIO: f32 = 0.62;

pub struct Meta<'a> {
    pub title: &'a str,
    pub first_line: u32,
}

pub struct Rendered {
    pub html: String,
    /// Canvas size in CSS pixels.
    pub width: u32,
    pub height: u32,
    pub lines: usize,
    /// Device pixel ratio used when the PNG is captured.
    pub scale: u32,
}

pub fn render(lines: &[Vec<Token>], options: &Options, theme: &Theme, meta: &Meta) -> Rendered {
    let font_size = options.font_size as f32;
    let char_w = font_size * CHAR_RATIO;
    let line_height = (font_size * 1.6).round().max(font_size + 4.0) as u32;
    let padding = options.padding as u32;
    let radius = options.radius;
    let scale = 2u32;

    let line_count = lines.len().max(1);
    let digits = (meta.first_line + line_count as u32 - 1)
        .to_string()
        .len()
        .max(2);
    let gutter = if options.line_numbers {
        (digits as f32 * char_w + 22.0).round() as u32
    } else {
        0
    };

    let longest = lines
        .iter()
        .map(|line| line_width(line))
        .max()
        .unwrap_or(1)
        .max(1);
    let code_width = longest as f32 * char_w;
    let title_height = if options.window { 38 } else { 0 };

    let card_width = padding as f32 * 2.0 + gutter as f32 + code_width + 2.0;
    let card_height =
        title_height as f32 + padding as f32 * 2.0 + line_count as f32 * line_height as f32 + 2.0;

    let (background, transparent) = background_css(&options.bg);
    // Enough room for the drop shadow, otherwise it gets clipped by the canvas.
    let margin = if options.shadow {
        56.0
    } else if transparent {
        0.0
    } else {
        28.0
    };

    // Two extra pixels keep the final row from being clipped by rounding.
    let width = (card_width + margin * 2.0).ceil() as u32 + 2;
    let height = (card_height + margin * 2.0).ceil() as u32 + 2;

    let mut html = String::with_capacity(2048 + lines.len() * 64);
    html.push_str("<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">");
    html.push_str("<meta name=\"generator\" content=\"zedpolacode\"><title>");
    push_escaped(&mut html, meta.title);
    html.push_str("</title><style>\n");

    html.push_str("*{box-sizing:border-box;margin:0;padding:0}\n");
    let _ = writeln!(
        html,
        "html,body{{width:{width}px;height:{height}px;overflow:hidden}}"
    );
    let _ = writeln!(
        html,
        "body{{display:flex;align-items:center;justify-content:center;background:{background};-webkit-font-smoothing:antialiased}}"
    );
    let shadow_css = if options.shadow {
        format!("box-shadow:{};", theme.shadow)
    } else {
        String::new()
    };
    let _ = writeln!(
        html,
        ".card{{background:{};border:1px solid {};border-radius:{radius}px;overflow:hidden;{shadow_css}}}",
        theme.bg, theme.border
    );
    if options.window {
        let _ = writeln!(
            html,
            ".chrome{{height:{title_height}px;display:flex;align-items:center;gap:7px;padding:0 16px;background:{};border-bottom:1px solid {};position:relative}}",
            theme.titlebar, theme.border
        );
        html.push_str(".dot{width:11px;height:11px;border-radius:50%}\n");
        html.push_str(
            ".dot.r{background:#ff5f57}.dot.y{background:#febc2e}.dot.g{background:#28c840}\n",
        );
        let _ = writeln!(
            html,
            ".title{{position:absolute;left:0;right:0;padding:0 78px;text-align:center;font:500 12px/1 {MONO};color:{};opacity:.85;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}}",
            theme.title_fg
        );
    }
    let _ = writeln!(html, ".code{{padding:{padding}px}}");
    let _ = writeln!(
        html,
        ".row{{display:flex;align-items:baseline;height:{line_height}px;line-height:{line_height}px}}"
    );
    if options.line_numbers {
        let _ = writeln!(
            html,
            ".ln{{flex:0 0 {gutter}px;width:{gutter}px;text-align:right;padding-right:20px;color:{};font-family:{MONO};font-size:{font_size}px;user-select:none;opacity:.8}}",
            theme.line_number
        );
    }
    let _ = writeln!(
        html,
        ".tx{{white-space:pre;font-family:{MONO};font-size:{font_size}px;color:{};font-variant-ligatures:none}}",
        theme.fg
    );
    let _ = writeln!(html, ".k{{color:{}}}", theme.keyword);
    let _ = writeln!(html, ".s{{color:{}}}", theme.string);
    let _ = writeln!(html, ".n{{color:{}}}", theme.number);
    let _ = writeln!(html, ".c{{color:{};font-style:italic}}", theme.comment);
    let _ = writeln!(html, ".f{{color:{}}}", theme.function);
    let _ = writeln!(html, ".t{{color:{}}}", theme.type_name);
    let _ = writeln!(html, ".v{{color:{}}}", theme.constant);

    html.push_str("</style></head><body><div class=\"card\">\n");

    if options.window {
        html.push_str("<div class=\"chrome\"><span class=\"dot r\"></span><span class=\"dot y\"></span><span class=\"dot g\"></span><span class=\"title\">");
        push_escaped(&mut html, meta.title);
        html.push_str("</span></div>\n");
    }

    html.push_str("<div class=\"code\">\n");
    for (index, line) in lines.iter().enumerate() {
        html.push_str("<div class=\"row\">");
        if options.line_numbers {
            let _ = write!(
                html,
                "<span class=\"ln\">{}</span>",
                meta.first_line + index as u32
            );
        }
        html.push_str("<span class=\"tx\">");
        if line.is_empty() {
            html.push_str("&#8203;");
        } else {
            for token in line {
                match class_for(token.kind) {
                    Some(class) => {
                        let _ = write!(html, "<span class=\"{class}\">");
                        push_escaped(&mut html, &token.text);
                        html.push_str("</span>");
                    }
                    None => push_escaped(&mut html, &token.text),
                }
            }
        }
        html.push_str("</span></div>\n");
    }
    html.push_str("</div>\n</div></body></html>");

    Rendered {
        html,
        width,
        height,
        lines: line_count,
        scale,
    }
}

fn class_for(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Keyword => Some("k"),
        Kind::Str => Some("s"),
        Kind::Num => Some("n"),
        Kind::Comment => Some("c"),
        Kind::Func => Some("f"),
        Kind::Type => Some("t"),
        Kind::Const => Some("v"),
        Kind::Plain | Kind::Punct => None,
    }
}

fn background_css(value: &str) -> (String, bool) {
    let normalized = value.trim().to_ascii_lowercase();
    let preset = match normalized.as_str() {
        "none" | "transparent" => return ("transparent".to_string(), true),
        "dark" => "#0b0d12",
        "light" => "#f5f6f8",
        "gradient" => "linear-gradient(135deg,#667eea 0%,#764ba2 100%)",
        "sunset" => "linear-gradient(135deg,#ff7e5f 0%,#feb47b 100%)",
        "ocean" => "linear-gradient(135deg,#2e3192 0%,#1bffff 100%)",
        "aurora" => "linear-gradient(135deg,#00c9ff 0%,#92fe9d 100%)",
        "midnight" => "linear-gradient(135deg,#0f2027 0%,#203a43 50%,#2c5364 100%)",
        "mono" => "linear-gradient(135deg,#232526 0%,#414345 100%)",
        other if other.starts_with('#') => other,
        _ => return ("transparent".to_string(), true),
    };
    (preset.to_string(), false)
}

fn push_escaped(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
}

fn char_columns(ch: char) -> usize {
    let code = ch as u32;
    let wide = (0x1100..=0x115F).contains(&code)
        || (0x2E80..=0xA4CF).contains(&code)
        || (0xAC00..=0xD7A3).contains(&code)
        || (0xF900..=0xFAFF).contains(&code)
        || (0xFE30..=0xFE6F).contains(&code)
        || (0xFF00..=0xFF60).contains(&code)
        || (0xFFE0..=0xFFE6).contains(&code)
        || (0x1F300..=0x1F64F).contains(&code)
        || (0x1F900..=0x1F9FF).contains(&code);
    if wide {
        2
    } else {
        1
    }
}

fn line_width(tokens: &[Token]) -> usize {
    tokens
        .iter()
        .map(|token| token.text.chars().map(char_columns).sum::<usize>())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::{language_for, tokenize};

    fn options() -> Options {
        Options::default()
    }

    #[test]
    fn escapes_html_and_wraps_tokens() {
        let tokens = tokenize("let s = \"<b>\";", language_for(Some("rust"), "x.rs"));
        let rendered = render(
            &tokens,
            &options(),
            crate::theme::default_theme(),
            &Meta {
                title: "x.rs",
                first_line: 1,
            },
        );
        assert!(rendered.html.contains("&lt;b&gt;"));
        assert!(!rendered.html.contains("\"<b>\""));
        assert!(rendered.html.contains("class=\"k\""));
        assert!(rendered.html.contains("class=\"row\""));
    }

    #[test]
    fn canvas_grows_with_longer_lines() {
        let short = render(
            &tokenize("a", language_for(Some("rust"), "a.rs")),
            &options(),
            crate::theme::default_theme(),
            &Meta {
                title: "a.rs",
                first_line: 1,
            },
        );
        let long = render(
            &tokenize(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                language_for(Some("rust"), "a.rs"),
            ),
            &options(),
            crate::theme::default_theme(),
            &Meta {
                title: "a.rs",
                first_line: 1,
            },
        );
        assert!(long.width > short.width);
        assert_eq!(short.height, long.height);
    }

    #[test]
    fn line_numbers_start_at_the_requested_line() {
        let rendered = render(
            &tokenize("a\nb", language_for(Some("rust"), "a.rs")),
            &options(),
            crate::theme::default_theme(),
            &Meta {
                title: "a.rs",
                first_line: 40,
            },
        );
        assert!(rendered.html.contains(">40</span>"));
        assert!(rendered.html.contains(">41</span>"));
    }

    #[test]
    fn background_presets() {
        assert_eq!(background_css("transparent").1, true);
        assert_eq!(background_css("#123456").0, "#123456");
        assert_eq!(background_css("gradient").1, false);
    }
}
