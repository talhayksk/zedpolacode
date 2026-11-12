//! Rendering options that are independent of any front end.
//!
//! The Zed extension builds these from slash-command flags; the MCP server
//! builds them from the tool call arguments.

/// Background presets accepted by `--bg` / the `bg` tool argument.
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
pub struct CardOptions {
    pub font_size: u32,
    pub padding: u32,
    pub radius: u32,
    pub line_numbers: bool,
    pub window: bool,
    pub shadow: bool,
    pub bg: String,
}

impl Default for CardOptions {
    fn default() -> Self {
        Self {
            font_size: 15,
            padding: 34,
            radius: 14,
            line_numbers: true,
            window: true,
            shadow: true,
            bg: "transparent".to_string(),
        }
    }
}

/// Accepts a preset name or a `#rgb`/`#rrggbb` style color.
pub fn valid_background(value: &str) -> bool {
    if BG_PRESETS.contains(&value) {
        return true;
    }
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_validation() {
        assert!(valid_background("transparent"));
        assert!(valid_background("gradient"));
        assert!(valid_background("#1e1e2e"));
        assert!(!valid_background("chartreuse"));
        assert!(!valid_background("#12345"));
    }
}
