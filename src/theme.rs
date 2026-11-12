//! Color palettes for the generated code cards.
//!
//! Every palette is a flat set of hex colors, so the produced HTML is
//! completely self-contained (no external CSS, no font downloads).

pub struct Theme {
    pub id: &'static str,
    pub label: &'static str,
    pub bg: &'static str,
    pub fg: &'static str,
    pub keyword: &'static str,
    pub string: &'static str,
    pub number: &'static str,
    pub comment: &'static str,
    pub function: &'static str,
    pub type_name: &'static str,
    pub constant: &'static str,
    pub line_number: &'static str,
    pub titlebar: &'static str,
    pub title_fg: &'static str,
    pub border: &'static str,
    pub shadow: &'static str,
}

const DARK_SHADOW: &str = "0 18px 40px rgba(0,0,0,.5), 0 4px 10px rgba(0,0,0,.28)";
const LIGHT_SHADOW: &str = "0 16px 36px rgba(15,23,42,.2), 0 4px 10px rgba(15,23,42,.12)";

pub static THEMES: &[Theme] = &[
    Theme {
        id: "dracula",
        label: "Dracula",
        bg: "#282a36",
        fg: "#f8f8f2",
        keyword: "#ff79c6",
        string: "#f1fa8c",
        number: "#bd93f9",
        comment: "#6272a4",
        function: "#50fa7b",
        type_name: "#8be9fd",
        constant: "#bd93f9",
        line_number: "#6272a4",
        titlebar: "#21222c",
        title_fg: "#a8aec9",
        border: "#44475a",
        shadow: DARK_SHADOW,
    },
    Theme {
        id: "one-dark",
        label: "One Dark",
        bg: "#282c34",
        fg: "#abb2bf",
        keyword: "#c678dd",
        string: "#98c379",
        number: "#d19a66",
        comment: "#5c6370",
        function: "#61afef",
        type_name: "#e5c07b",
        constant: "#d19a66",
        line_number: "#4b5263",
        titlebar: "#21252b",
        title_fg: "#9da5b4",
        border: "#3e4451",
        shadow: DARK_SHADOW,
    },
    Theme {
        id: "nord",
        label: "Nord",
        bg: "#2e3440",
        fg: "#d8dee9",
        keyword: "#81a1c1",
        string: "#a3be8c",
        number: "#b48ead",
        comment: "#616e88",
        function: "#88c0d0",
        type_name: "#8fbcbb",
        constant: "#d08770",
        line_number: "#4c566a",
        titlebar: "#272c36",
        title_fg: "#a9b3c4",
        border: "#3b4252",
        shadow: DARK_SHADOW,
    },
    Theme {
        id: "tokyo-night",
        label: "Tokyo Night",
        bg: "#1a1b26",
        fg: "#c0caf5",
        keyword: "#bb9af7",
        string: "#9ece6a",
        number: "#ff9e64",
        comment: "#565f89",
        function: "#7aa2f7",
        type_name: "#2ac3de",
        constant: "#e0af68",
        line_number: "#3b4261",
        titlebar: "#16161e",
        title_fg: "#9aa5ce",
        border: "#2f334d",
        shadow: DARK_SHADOW,
    },
    Theme {
        id: "github-dark",
        label: "GitHub Dark",
        bg: "#0d1117",
        fg: "#c9d1d9",
        keyword: "#ff7b72",
        string: "#a5d6ff",
        number: "#79c0ff",
        comment: "#8b949e",
        function: "#d2a8ff",
        type_name: "#ffa657",
        constant: "#79c0ff",
        line_number: "#484f58",
        titlebar: "#161b22",
        title_fg: "#8b949e",
        border: "#30363d",
        shadow: DARK_SHADOW,
    },
    Theme {
        id: "catppuccin-mocha",
        label: "Catppuccin Mocha",
        bg: "#1e1e2e",
        fg: "#cdd6f4",
        keyword: "#cba6f7",
        string: "#a6e3a1",
        number: "#fab387",
        comment: "#6c7086",
        function: "#89b4fa",
        type_name: "#f9e2af",
        constant: "#f5c2e7",
        line_number: "#585b70",
        titlebar: "#181825",
        title_fg: "#a6adc8",
        border: "#313244",
        shadow: DARK_SHADOW,
    },
    Theme {
        id: "github-light",
        label: "GitHub Light",
        bg: "#ffffff",
        fg: "#24292f",
        keyword: "#cf222e",
        string: "#0a3069",
        number: "#0550ae",
        comment: "#6e7781",
        function: "#8250df",
        type_name: "#953800",
        constant: "#0550ae",
        line_number: "#8c959f",
        titlebar: "#f6f8fa",
        title_fg: "#57606a",
        border: "#d0d7de",
        shadow: LIGHT_SHADOW,
    },
    Theme {
        id: "solarized-light",
        label: "Solarized Light",
        bg: "#fdf6e3",
        fg: "#4d5b63",
        keyword: "#859900",
        string: "#2aa198",
        number: "#d33682",
        comment: "#93a1a1",
        function: "#268bd2",
        type_name: "#b58900",
        constant: "#cb4b16",
        line_number: "#93a1a1",
        titlebar: "#eee8d5",
        title_fg: "#839496",
        border: "#e3dcc5",
        shadow: LIGHT_SHADOW,
    },
];

/// Shortcuts people actually type.
static ALIASES: &[(&str, &str)] = &[
    ("onedark", "one-dark"),
    ("one", "one-dark"),
    ("tokyonight", "tokyo-night"),
    ("tokyo", "tokyo-night"),
    ("githubdark", "github-dark"),
    ("gh-dark", "github-dark"),
    ("githublight", "github-light"),
    ("gh-light", "github-light"),
    ("catppuccin", "catppuccin-mocha"),
    ("mocha", "catppuccin-mocha"),
    ("solarized", "solarized-light"),
    ("dark", "one-dark"),
    ("light", "github-light"),
];

fn normalize(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(['_', ' ', '.'], "-")
}

pub fn resolve(name: &str) -> Option<&'static Theme> {
    let want = normalize(name);
    let want = ALIASES
        .iter()
        .find(|(alias, _)| *alias == want)
        .map(|(_, target)| target.to_string())
        .unwrap_or(want);

    THEMES
        .iter()
        .find(|theme| theme.id == want || normalize(theme.label) == want)
}

pub fn default_theme() -> &'static Theme {
    &THEMES[0]
}

pub fn ids() -> Vec<&'static str> {
    THEMES.iter().map(|theme| theme.id).collect()
}

pub fn id_list() -> String {
    THEMES
        .iter()
        .map(|theme| theme.id)
        .collect::<Vec<_>>()
        .join(", ")
}
