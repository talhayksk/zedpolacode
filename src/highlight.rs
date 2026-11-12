//! A small, dependency-free syntax highlighter.
//!
//! It is not a parser: it scans for comments, strings, numbers and words, then
//! classifies words with a per-language keyword list plus a few heuristics
//! (`foo(` is a call, `Foo` is a type, ...). That is more than enough to make a
//! screenshot look like the real editor.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Plain,
    Keyword,
    Str,
    Num,
    Comment,
    Func,
    Type,
    Const,
    Punct,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: Kind,
    pub text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Regular programming language.
    Code,
    /// HTML/XML: tag names, attributes and values.
    Markup,
    /// CSS/SCSS: selectors, properties, at-rules.
    Css,
    /// YAML/TOML: `key: value` / `key = value`.
    KeyValue,
    /// JSON: quoted keys.
    Data,
}

pub struct Language {
    pub id: &'static str,
    pub aliases: &'static [&'static str],
    pub style: Style,
    pub keywords: &'static [&'static str],
    pub line_comments: &'static [&'static str],
    pub block_comment: bool,
    pub quotes: &'static [char],
    /// Quote characters that may span multiple lines (backticks, ...).
    pub raw_quotes: &'static [char],
    pub triple_quotes: bool,
}

const CONSTANTS: &[&str] = &[
    "true",
    "false",
    "null",
    "nil",
    "None",
    "True",
    "False",
    "undefined",
    "NaN",
    "Infinity",
];

const RUST_KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
    "union",
    "yield",
    "box",
    "macro_rules",
    "macro",
];

const PYTHON_KEYWORDS: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda",
    "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "match",
    "case", "print",
];

const JS_KEYWORDS: &[&str] = &[
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "of",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "interface",
    "type",
    "enum",
    "implements",
    "readonly",
    "namespace",
    "declare",
    "abstract",
    "public",
    "private",
    "protected",
    "as",
    "satisfies",
    "keyof",
    "infer",
    "never",
    "unknown",
    "any",
    "string",
    "number",
    "boolean",
    "export default",
];

const GO_KEYWORDS: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
];

const C_KEYWORDS: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
    "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
    "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
    "union", "unsigned", "void", "volatile", "while", "include", "define", "ifdef", "ifndef",
    "endif", "pragma",
];

const CPP_KEYWORDS: &[&str] = &[
    "auto",
    "break",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "constexpr",
    "continue",
    "default",
    "delete",
    "do",
    "double",
    "else",
    "enum",
    "explicit",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "nullptr",
    "operator",
    "override",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "sizeof",
    "static",
    "struct",
    "switch",
    "template",
    "this",
    "throw",
    "true",
    "try",
    "typedef",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "while",
];

const CSHARP_KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "async",
    "await",
    "base",
    "bool",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "decimal",
    "default",
    "delegate",
    "do",
    "double",
    "else",
    "enum",
    "event",
    "explicit",
    "extern",
    "finally",
    "fixed",
    "float",
    "for",
    "foreach",
    "get",
    "goto",
    "if",
    "implicit",
    "in",
    "int",
    "interface",
    "internal",
    "is",
    "lock",
    "long",
    "namespace",
    "new",
    "object",
    "operator",
    "out",
    "override",
    "params",
    "private",
    "protected",
    "public",
    "readonly",
    "record",
    "ref",
    "return",
    "sealed",
    "set",
    "short",
    "sizeof",
    "static",
    "string",
    "struct",
    "switch",
    "this",
    "throw",
    "try",
    "typeof",
    "uint",
    "ulong",
    "unsafe",
    "ushort",
    "using",
    "var",
    "virtual",
    "void",
    "volatile",
    "while",
    "yield",
    "init",
    "nameof",
    "when",
    "where",
];

const JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "sealed",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "var",
    "void",
    "volatile",
    "while",
];

const KOTLIN_KEYWORDS: &[&str] = &[
    "fun",
    "val",
    "var",
    "class",
    "object",
    "interface",
    "data",
    "sealed",
    "when",
    "if",
    "else",
    "for",
    "while",
    "do",
    "return",
    "break",
    "continue",
    "try",
    "catch",
    "finally",
    "throw",
    "import",
    "package",
    "is",
    "as",
    "in",
    "out",
    "typealias",
    "companion",
    "init",
    "constructor",
    "override",
    "open",
    "abstract",
    "private",
    "public",
    "protected",
    "internal",
    "lateinit",
    "by",
    "suspend",
    "inline",
    "reified",
    "where",
    "this",
    "super",
    "it",
];

const SWIFT_KEYWORDS: &[&str] = &[
    "associatedtype",
    "class",
    "deinit",
    "enum",
    "extension",
    "fileprivate",
    "func",
    "import",
    "init",
    "inout",
    "internal",
    "let",
    "open",
    "operator",
    "private",
    "protocol",
    "public",
    "rethrows",
    "static",
    "struct",
    "subscript",
    "typealias",
    "var",
    "break",
    "case",
    "catch",
    "continue",
    "default",
    "defer",
    "do",
    "else",
    "fallthrough",
    "for",
    "guard",
    "if",
    "in",
    "repeat",
    "return",
    "switch",
    "throw",
    "try",
    "where",
    "while",
    "as",
    "self",
    "Self",
    "super",
    "async",
    "await",
    "some",
    "any",
];

const RUBY_KEYWORDS: &[&str] = &[
    "alias",
    "and",
    "begin",
    "break",
    "case",
    "class",
    "def",
    "defined?",
    "do",
    "else",
    "elsif",
    "end",
    "ensure",
    "for",
    "if",
    "in",
    "module",
    "next",
    "not",
    "or",
    "redo",
    "rescue",
    "retry",
    "return",
    "self",
    "super",
    "then",
    "undef",
    "unless",
    "until",
    "when",
    "while",
    "yield",
    "require",
    "require_relative",
    "attr_accessor",
    "attr_reader",
    "puts",
];

const PHP_KEYWORDS: &[&str] = &[
    "abstract",
    "and",
    "array",
    "as",
    "break",
    "callable",
    "case",
    "catch",
    "class",
    "clone",
    "const",
    "continue",
    "declare",
    "default",
    "do",
    "echo",
    "else",
    "elseif",
    "empty",
    "enddeclare",
    "endfor",
    "endforeach",
    "endif",
    "endswitch",
    "endwhile",
    "extends",
    "final",
    "finally",
    "fn",
    "for",
    "foreach",
    "function",
    "global",
    "goto",
    "if",
    "implements",
    "include",
    "include_once",
    "instanceof",
    "insteadof",
    "interface",
    "isset",
    "list",
    "match",
    "namespace",
    "new",
    "or",
    "print",
    "private",
    "protected",
    "public",
    "readonly",
    "require",
    "require_once",
    "return",
    "static",
    "switch",
    "throw",
    "trait",
    "try",
    "unset",
    "use",
    "while",
    "xor",
    "yield",
];

const SHELL_KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac",
    "function", "in", "select", "time", "return", "exit", "export", "local", "readonly", "declare",
    "unset", "shift", "source", "alias", "echo", "set", "eval", "exec", "trap",
];

const SQL_KEYWORDS: &[&str] = &[
    "select",
    "from",
    "where",
    "insert",
    "into",
    "values",
    "update",
    "set",
    "delete",
    "create",
    "table",
    "drop",
    "alter",
    "add",
    "index",
    "view",
    "join",
    "inner",
    "left",
    "right",
    "full",
    "outer",
    "on",
    "group",
    "by",
    "order",
    "having",
    "limit",
    "offset",
    "union",
    "all",
    "distinct",
    "as",
    "and",
    "or",
    "not",
    "is",
    "in",
    "exists",
    "case",
    "when",
    "then",
    "else",
    "end",
    "primary",
    "key",
    "foreign",
    "references",
    "default",
    "constraint",
    "unique",
    "with",
    "returning",
    "using",
    "asc",
    "desc",
    "begin",
    "commit",
    "rollback",
    "transaction",
    "select",
    "SELECT",
    "FROM",
    "WHERE",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE",
    "TABLE",
    "DROP",
    "ALTER",
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "OUTER",
    "ON",
    "GROUP",
    "BY",
    "ORDER",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "UNION",
    "ALL",
    "DISTINCT",
    "AS",
    "AND",
    "OR",
    "NOT",
    "IS",
    "IN",
    "EXISTS",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "PRIMARY",
    "KEY",
    "FOREIGN",
    "REFERENCES",
    "DEFAULT",
    "CONSTRAINT",
    "UNIQUE",
    "WITH",
    "RETURNING",
    "USING",
    "ASC",
    "DESC",
];

const CSS_KEYWORDS: &[&str] = &[
    "media",
    "import",
    "supports",
    "keyframes",
    "font-face",
    "from",
    "to",
    "important",
    "root",
    "hover",
    "focus",
    "active",
    "before",
    "after",
    "not",
    "is",
    "where",
    "has",
    "nth-child",
];

const CONFIG_KEYWORDS: &[&str] = &["true", "false", "null", "yes", "no", "on", "off"];

const LUA_KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "for", "function", "goto", "if", "in", "local",
    "not", "or", "repeat", "return", "then", "until", "while",
];

const ZIG_KEYWORDS: &[&str] = &[
    "const",
    "var",
    "fn",
    "pub",
    "if",
    "else",
    "while",
    "for",
    "switch",
    "return",
    "struct",
    "enum",
    "union",
    "comptime",
    "try",
    "catch",
    "async",
    "await",
    "defer",
    "errdefer",
    "orelse",
    "break",
    "continue",
    "test",
    "usingnamespace",
    "export",
    "extern",
    "inline",
    "noinline",
];

static PLAINTEXT: Language = Language {
    id: "plaintext",
    aliases: &[
        "txt", "text", "plain", "log", "output", "md", "markdown", "diff", "patch",
    ],
    style: Style::Code,
    keywords: &[],
    line_comments: &[],
    block_comment: false,
    quotes: &[],
    raw_quotes: &[],
    triple_quotes: false,
};

static RUST: Language = Language {
    id: "rust",
    aliases: &["rs"],
    style: Style::Code,
    keywords: RUST_KEYWORDS,
    line_comments: &["//"],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static PYTHON: Language = Language {
    id: "python",
    aliases: &["py", "python3", "jupyter"],
    style: Style::Code,
    keywords: PYTHON_KEYWORDS,
    line_comments: &["#"],
    block_comment: false,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: true,
};

static JAVASCRIPT: Language = Language {
    id: "javascript",
    aliases: &["js", "jsx", "mjs", "cjs", "node"],
    style: Style::Code,
    keywords: JS_KEYWORDS,
    line_comments: &["//"],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &['`'],
    triple_quotes: false,
};

static TYPESCRIPT: Language = Language {
    id: "typescript",
    aliases: &["ts", "tsx", "mts", "cts", "typescriptreact"],
    style: Style::Code,
    keywords: JS_KEYWORDS,
    line_comments: &["//"],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &['`'],
    triple_quotes: false,
};

static GO: Language = Language {
    id: "go",
    aliases: &["golang"],
    style: Style::Code,
    keywords: GO_KEYWORDS,
    line_comments: &["//"],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &['`'],
    triple_quotes: false,
};

static C: Language = Language {
    id: "c",
    aliases: &["h"],
    style: Style::Code,
    keywords: C_KEYWORDS,
    line_comments: &["//"],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static CPP: Language = Language {
    id: "cpp",
    aliases: &["c++", "cc", "cxx", "hpp", "hh", "hxx"],
    style: Style::Code,
    keywords: CPP_KEYWORDS,
    line_comments: &["//"],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static CSHARP: Language = Language {
    id: "csharp",
    aliases: &["cs", "c#", "dotnet"],
    style: Style::Code,
    keywords: CSHARP_KEYWORDS,
    line_comments: &["//"],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static JAVA: Language = Language {
    id: "java",
    aliases: &[],
    style: Style::Code,
    keywords: JAVA_KEYWORDS,
    line_comments: &["//"],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static KOTLIN: Language = Language {
    id: "kotlin",
    aliases: &["kt", "kts"],
    style: Style::Code,
    keywords: KOTLIN_KEYWORDS,
    line_comments: &["//"],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: true,
};

static SWIFT: Language = Language {
    id: "swift",
    aliases: &[],
    style: Style::Code,
    keywords: SWIFT_KEYWORDS,
    line_comments: &["//"],
    block_comment: true,
    quotes: &['"'],
    raw_quotes: &[],
    triple_quotes: true,
};

static RUBY: Language = Language {
    id: "ruby",
    aliases: &["rb"],
    style: Style::Code,
    keywords: RUBY_KEYWORDS,
    line_comments: &["#"],
    block_comment: false,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static PHP: Language = Language {
    id: "php",
    aliases: &[],
    style: Style::Code,
    keywords: PHP_KEYWORDS,
    line_comments: &["//", "#"],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static SHELL: Language = Language {
    id: "shell",
    aliases: &[
        "sh",
        "bash",
        "zsh",
        "fish",
        "console",
        "shellscript",
        "zshrc",
        "bashrc",
    ],
    style: Style::Code,
    keywords: SHELL_KEYWORDS,
    line_comments: &["#"],
    block_comment: false,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static SQL: Language = Language {
    id: "sql",
    aliases: &["postgres", "postgresql", "mysql", "sqlite", "psql"],
    style: Style::Code,
    keywords: SQL_KEYWORDS,
    line_comments: &["--"],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static LUA: Language = Language {
    id: "lua",
    aliases: &[],
    style: Style::Code,
    keywords: LUA_KEYWORDS,
    line_comments: &["--"],
    block_comment: false,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static ZIG: Language = Language {
    id: "zig",
    aliases: &[],
    style: Style::Code,
    keywords: ZIG_KEYWORDS,
    line_comments: &["//"],
    block_comment: false,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static HTML: Language = Language {
    id: "html",
    aliases: &[
        "markup",
        "xml",
        "svg",
        "htm",
        "vue",
        "svelte",
        "astro",
        "jinja",
        "handlebars",
    ],
    style: Style::Markup,
    keywords: &[],
    line_comments: &[],
    block_comment: false,
    quotes: &[],
    raw_quotes: &[],
    triple_quotes: false,
};

static CSS: Language = Language {
    id: "css",
    aliases: &["scss", "sass", "less", "stylus"],
    style: Style::Css,
    keywords: CSS_KEYWORDS,
    line_comments: &[],
    block_comment: true,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static JSON: Language = Language {
    id: "json",
    aliases: &["jsonc", "json5", "ipynb"],
    style: Style::Data,
    keywords: &[],
    line_comments: &[],
    block_comment: false,
    quotes: &['"'],
    raw_quotes: &[],
    triple_quotes: false,
};

static YAML: Language = Language {
    id: "yaml",
    aliases: &["yml"],
    style: Style::KeyValue,
    keywords: CONFIG_KEYWORDS,
    line_comments: &["#"],
    block_comment: false,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

static TOML: Language = Language {
    id: "toml",
    aliases: &["ini", "conf", "cfg", "env", "properties"],
    style: Style::KeyValue,
    keywords: CONFIG_KEYWORDS,
    line_comments: &["#", ";"],
    block_comment: false,
    quotes: &['"', '\''],
    raw_quotes: &[],
    triple_quotes: false,
};

pub static LANGUAGES: &[&Language] = &[
    &RUST,
    &PYTHON,
    &JAVASCRIPT,
    &TYPESCRIPT,
    &GO,
    &C,
    &CPP,
    &CSHARP,
    &JAVA,
    &KOTLIN,
    &SWIFT,
    &RUBY,
    &PHP,
    &SHELL,
    &SQL,
    &LUA,
    &ZIG,
    &HTML,
    &CSS,
    &JSON,
    &YAML,
    &TOML,
    &PLAINTEXT,
];

fn find(key: &str) -> Option<&'static Language> {
    if key.is_empty() {
        return None;
    }
    LANGUAGES
        .iter()
        .copied()
        .find(|lang| lang.id == key || lang.aliases.iter().any(|alias| *alias == key))
}

/// Resolves the language from an explicit hint, falling back to the file extension.
pub fn language_for(hint: Option<&str>, label: &str) -> &'static Language {
    if let Some(hint) = hint {
        let key = hint.trim().trim_start_matches('.').to_ascii_lowercase();
        if let Some(lang) = find(&key) {
            return lang;
        }
    }

    let file = label
        .rsplit(|ch| ch == '/' || ch == '\\')
        .next()
        .unwrap_or(label);
    let ext = match file.rsplit_once('.') {
        Some((_, ext)) => ext.to_ascii_lowercase(),
        None => String::new(),
    };

    find(&ext).unwrap_or(&PLAINTEXT)
}

pub fn id_list() -> String {
    LANGUAGES
        .iter()
        .filter(|lang| lang.id != "plaintext")
        .map(|lang| lang.id)
        .collect::<Vec<_>>()
        .join(", ")
}

fn starts_at(chars: &[char], at: usize, pattern: &str) -> bool {
    let mut index = at;
    for expected in pattern.chars() {
        if chars.get(index) != Some(&expected) {
            return false;
        }
        index += 1;
    }
    true
}

fn is_ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_' || ch == '$'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}

fn next_significant(chars: &[char], from: usize) -> Option<char> {
    let mut index = from;
    while let Some(&ch) = chars.get(index) {
        match ch {
            ' ' | '\t' => index += 1,
            '\n' => return None,
            other => return Some(other),
        }
    }
    None
}

fn classify(word: &str, chars: &[char], next: usize, lang: &'static Language) -> Kind {
    if lang.keywords.contains(&word) {
        return Kind::Keyword;
    }
    if CONSTANTS.contains(&word) {
        return Kind::Const;
    }
    if lang.style != Style::Code {
        return Kind::Plain;
    }
    if next_significant(chars, next) == Some('(') {
        return Kind::Func;
    }
    if word.chars().next().map(char::is_uppercase).unwrap_or(false) {
        return Kind::Type;
    }
    Kind::Plain
}

fn scan_number(chars: &[char], start: usize) -> usize {
    let len = chars.len();
    let mut index = start;

    if chars[index] == '0'
        && matches!(
            chars.get(index + 1),
            Some('x' | 'X' | 'b' | 'B' | 'o' | 'O')
        )
    {
        index += 2;
        while index < len && (chars[index].is_ascii_alphanumeric() || chars[index] == '_') {
            index += 1;
        }
        return index;
    }

    while index < len && (chars[index].is_ascii_digit() || chars[index] == '_') {
        index += 1;
    }

    if chars.get(index) == Some(&'.')
        && chars
            .get(index + 1)
            .map(|digit| digit.is_ascii_digit())
            .unwrap_or(false)
    {
        index += 1;
        while index < len && (chars[index].is_ascii_digit() || chars[index] == '_') {
            index += 1;
        }
    }

    if matches!(chars.get(index), Some('e' | 'E')) {
        let mut probe = index + 1;
        if matches!(chars.get(probe), Some('+' | '-')) {
            probe += 1;
        }
        if chars
            .get(probe)
            .map(|digit| digit.is_ascii_digit())
            .unwrap_or(false)
        {
            index = probe;
            while index < len && chars[index].is_ascii_digit() {
                index += 1;
            }
        }
    }

    // Type suffixes (`f32`, `u8`, `LL`) stay part of the literal.
    while index < len && (chars[index].is_ascii_alphanumeric() || chars[index] == '_') {
        index += 1;
    }

    index
}

fn push_token(current: &mut Vec<Token>, lines: &mut Vec<Vec<Token>>, kind: Kind, text: &[char]) {
    let mut segment = String::new();
    for ch in text {
        if *ch == '\n' {
            if !segment.is_empty() {
                push_segment(current, kind, &segment);
                segment.clear();
            }
            lines.push(std::mem::take(current));
            continue;
        }
        segment.push(*ch);
    }
    if !segment.is_empty() {
        push_segment(current, kind, &segment);
    }
}

fn push_segment(current: &mut Vec<Token>, kind: Kind, text: &str) {
    // Plain runs are kept in a single token, but two separate plain runs are
    // never merged: that keeps punctuation next to a word (`key:`, `color:`)
    // discoverable by the post-pass.
    if kind != Kind::Plain {
        if let Some(last) = current.last_mut() {
            if last.kind == kind {
                last.text.push_str(text);
                return;
            }
        }
    }
    current.push(Token {
        kind,
        text: text.to_string(),
    });
}

fn scan_markup_tag(
    chars: &[char],
    start: usize,
    current: &mut Vec<Token>,
    lines: &mut Vec<Vec<Token>>,
) -> usize {
    let len = chars.len();
    let mut index = start;

    push_token(current, lines, Kind::Punct, &chars[index..index + 1]);
    index += 1;

    if index < len && chars[index] == '/' {
        push_token(current, lines, Kind::Punct, &chars[index..index + 1]);
        index += 1;
    }

    if index < len && chars[index] == '!' {
        let begin = index;
        while index < len && chars[index] != '>' {
            index += 1;
        }
        push_token(current, lines, Kind::Punct, &chars[begin..index]);
    }

    let begin = index;
    while index < len
        && (chars[index].is_alphanumeric() || matches!(chars[index], '-' | '_' | ':' | '.'))
    {
        index += 1;
    }
    if index > begin {
        push_token(current, lines, Kind::Keyword, &chars[begin..index]);
    }

    while index < len {
        let ch = chars[index];
        if ch == '>' {
            push_token(current, lines, Kind::Punct, &chars[index..index + 1]);
            index += 1;
            break;
        }
        if ch == '"' || ch == '\'' {
            let begin = index;
            index += 1;
            while index < len && chars[index] != ch {
                index += 1;
            }
            if index < len {
                index += 1;
            }
            push_token(current, lines, Kind::Str, &chars[begin..index]);
            continue;
        }
        if ch.is_alphanumeric() || matches!(ch, '-' | '_' | ':') {
            let begin = index;
            while index < len
                && (chars[index].is_alphanumeric() || matches!(chars[index], '-' | '_' | ':'))
            {
                index += 1;
            }
            push_token(current, lines, Kind::Type, &chars[begin..index]);
            continue;
        }
        push_token(current, lines, Kind::Punct, &chars[index..index + 1]);
        index += 1;
    }

    index
}

fn first_content(tokens: &[Token]) -> Option<usize> {
    tokens
        .iter()
        .position(|token| !token.text.trim().is_empty())
}

fn nth_content(tokens: &[Token], skip: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .skip(skip)
        .find(|(_, token)| !token.text.trim().is_empty())
        .map(|(index, _)| index)
}

fn mark_key_value(line: &mut Vec<Token>) {
    let Some(mut index) = first_content(line) else {
        return;
    };

    // `[section]` headers in TOML/INI.
    if line[index].text.trim_start().starts_with('[') {
        let mut in_header = true;
        for token in line.iter_mut() {
            if in_header {
                if token.kind == Kind::Plain {
                    token.kind = Kind::Keyword;
                }
                if token.text.contains(']') {
                    in_header = false;
                }
            }
        }
        return;
    }

    if line[index].text == "-" {
        match nth_content(line, index + 1) {
            Some(next) => index = next,
            None => return,
        }
    }

    if line[index].kind != Kind::Plain {
        return;
    }
    let word = line[index].text.trim();
    if word.is_empty()
        || !word
            .chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        return;
    }

    if let Some(next) = nth_content(line, index + 1) {
        let next_text = line[next].text.trim_start();
        if next_text.starts_with(':') || next_text.starts_with('=') {
            line[index].kind = Kind::Keyword;
        }
    }
}

fn mark_css(line: &mut Vec<Token>) {
    for index in 0..line.len() {
        if line[index].kind != Kind::Plain {
            continue;
        }
        let text = line[index].text.clone();
        let word = text.trim();
        if word.is_empty()
            || !word
                .chars()
                .next()
                .map(|ch| ch.is_alphabetic() || ch == '-')
                .unwrap_or(false)
            || !word
                .chars()
                .all(|ch| ch.is_alphanumeric() || matches!(ch, '-' | '_'))
        {
            continue;
        }

        let after_at = line[..index]
            .iter()
            .rev()
            .find(|token| !token.text.trim().is_empty())
            .map(|token| token.text.trim_end().ends_with('@'))
            .unwrap_or(false);
        if after_at {
            line[index].kind = Kind::Keyword;
            continue;
        }

        if let Some(next) = nth_content(line, index + 1) {
            if line[next].text.trim_start().starts_with(':') {
                line[index].kind = Kind::Keyword;
            }
        }
    }
}

fn mark_json_keys(line: &mut Vec<Token>) {
    for index in 0..line.len() {
        if line[index].kind != Kind::Str {
            continue;
        }
        if let Some(next) = nth_content(line, index + 1) {
            if line[next].text.trim_start().starts_with(':') {
                line[index].kind = Kind::Keyword;
            }
        }
    }
}

fn postprocess(lines: &mut [Vec<Token>], style: Style) {
    match style {
        Style::KeyValue => {
            for line in lines.iter_mut() {
                mark_key_value(line);
            }
        }
        Style::Css => {
            for line in lines.iter_mut() {
                mark_css(line);
            }
        }
        Style::Data => {
            for line in lines.iter_mut() {
                mark_json_keys(line);
            }
        }
        _ => {}
    }
}

/// Splits `source` into highlighted lines.
pub fn tokenize(source: &str, lang: &'static Language) -> Vec<Vec<Token>> {
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut lines: Vec<Vec<Token>> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    let mut index = 0usize;

    while index < len {
        let ch = chars[index];

        if ch == '\n' {
            lines.push(std::mem::take(&mut current));
            index += 1;
            continue;
        }

        let mut matched_comment = false;
        for marker in lang.line_comments {
            if starts_at(&chars, index, marker) {
                let begin = index;
                while index < len && chars[index] != '\n' {
                    index += 1;
                }
                push_token(
                    &mut current,
                    &mut lines,
                    Kind::Comment,
                    &chars[begin..index],
                );
                matched_comment = true;
                break;
            }
        }
        if matched_comment {
            continue;
        }

        if lang.block_comment && ch == '/' && chars.get(index + 1) == Some(&'*') {
            let begin = index;
            index += 2;
            while index < len {
                if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                    index += 2;
                    break;
                }
                index += 1;
            }
            push_token(
                &mut current,
                &mut lines,
                Kind::Comment,
                &chars[begin..index],
            );
            continue;
        }

        if lang.style == Style::Markup && starts_at(&chars, index, "<!--") {
            let begin = index;
            index += 4;
            while index < len && !starts_at(&chars, index, "-->") {
                index += 1;
            }
            index = (index + 3).min(len);
            push_token(
                &mut current,
                &mut lines,
                Kind::Comment,
                &chars[begin..index],
            );
            continue;
        }

        if lang.triple_quotes {
            let delim = if starts_at(&chars, index, "\"\"\"") {
                Some("\"\"\"")
            } else if starts_at(&chars, index, "'''") {
                Some("'''")
            } else {
                None
            };
            if let Some(delim) = delim {
                let begin = index;
                index += 3;
                while index < len {
                    if chars[index] == '\\' {
                        index = (index + 2).min(len);
                        continue;
                    }
                    if starts_at(&chars, index, delim) {
                        index += 3;
                        break;
                    }
                    index += 1;
                }
                push_token(&mut current, &mut lines, Kind::Str, &chars[begin..index]);
                continue;
            }
        }

        // Rust lifetimes (`'a`, `'static`) must not be read as char literals.
        if ch == '\''
            && lang.id == "rust"
            && chars
                .get(index + 1)
                .copied()
                .map(is_ident_start)
                .unwrap_or(false)
            && chars.get(index + 2) != Some(&'\'')
        {
            let begin = index;
            index += 1;
            while index < len && is_ident_continue(chars[index]) {
                index += 1;
            }
            push_token(&mut current, &mut lines, Kind::Type, &chars[begin..index]);
            continue;
        }

        if lang.quotes.contains(&ch) || lang.raw_quotes.contains(&ch) {
            let multiline = lang.raw_quotes.contains(&ch);
            let begin = index;
            index += 1;
            while index < len {
                let inner = chars[index];
                if inner == '\\' {
                    index = (index + 2).min(len);
                    continue;
                }
                if inner == ch {
                    index += 1;
                    break;
                }
                if inner == '\n' && !multiline {
                    break;
                }
                index += 1;
            }
            push_token(&mut current, &mut lines, Kind::Str, &chars[begin..index]);
            continue;
        }

        if lang.style == Style::Markup && ch == '<' {
            index = scan_markup_tag(&chars, index, &mut current, &mut lines);
            continue;
        }

        if ch.is_ascii_digit() {
            let begin = index;
            index = scan_number(&chars, index);
            push_token(&mut current, &mut lines, Kind::Num, &chars[begin..index]);
            continue;
        }

        if is_ident_start(ch) {
            let begin = index;
            while index < len && is_ident_continue(chars[index]) {
                index += 1;
            }
            if lang.id == "ruby" && chars.get(index) == Some(&'!') {
                index += 1;
            }
            let word: String = chars[begin..index].iter().collect();
            let kind = classify(&word, &chars, index, lang);
            push_token(&mut current, &mut lines, kind, &chars[begin..index]);
            continue;
        }

        push_token(
            &mut current,
            &mut lines,
            Kind::Plain,
            &chars[index..index + 1],
        );
        index += 1;
    }

    if !current.is_empty() {
        lines.push(current);
    }

    postprocess(&mut lines, lang.style);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(source: &str, lang: &'static Language) -> Vec<Token> {
        tokenize(source, lang).into_iter().flatten().collect()
    }

    fn kind_of(tokens: &[Token], needle: &str) -> Option<Kind> {
        tokens
            .iter()
            .find(|token| token.text == needle)
            .map(|token| token.kind)
    }

    #[test]
    fn splits_lines_and_highlights_rust() {
        let lines = tokenize("fn main() {\n    let x: u32 = 42; // hi\n}", &RUST);
        assert_eq!(lines.len(), 3);
        let tokens = flat("fn main() {\n    let x: u32 = 42; // hi\n}", &RUST);
        assert_eq!(kind_of(&tokens, "fn"), Some(Kind::Keyword));
        assert_eq!(kind_of(&tokens, "main"), Some(Kind::Func));
        assert_eq!(kind_of(&tokens, "42"), Some(Kind::Num));
        assert!(tokens.iter().any(|token| token.kind == Kind::Comment));
    }

    #[test]
    fn rust_lifetime_is_not_a_char_literal() {
        let tokens = flat("fn f<'a>(x: &'a str) {}", &RUST);
        assert_eq!(kind_of(&tokens, "'a"), Some(Kind::Type));
        assert_eq!(kind_of(&tokens, "&"), Some(Kind::Plain));
    }

    #[test]
    fn multiline_comment_stays_one_token_across_lines() {
        let lines = tokenize("/* one\n   two */ let x = 1;", &JAVASCRIPT);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].iter().all(|token| token.kind == Kind::Comment));
        assert!(lines[1].iter().any(|token| token.kind == Kind::Keyword));
    }

    #[test]
    fn python_triple_quotes_are_strings() {
        let lines = tokenize("x = \"\"\"a\nb\"\"\"\n", &PYTHON);
        assert_eq!(lines.len(), 2);
        assert!(lines[0]
            .iter()
            .any(|token| token.kind == Kind::Str && token.text == "\"\"\"a"));
        assert!(lines[1]
            .iter()
            .any(|token| token.kind == Kind::Str && token.text == "b\"\"\""));
    }

    #[test]
    fn json_keys_are_keywords() {
        let tokens = flat("{\"name\": \"zed\", \"line\": 3}", &JSON);
        assert_eq!(kind_of(&tokens, "\"name\""), Some(Kind::Keyword));
        assert_eq!(kind_of(&tokens, "\"zed\""), Some(Kind::Str));
        assert_eq!(kind_of(&tokens, "3"), Some(Kind::Num));
    }

    #[test]
    fn yaml_keys_and_comments() {
        let tokens = flat("name: polacode # comment", &YAML);
        let debug: Vec<String> = tokens
            .iter()
            .map(|token| format!("{:?}:{}", token.kind, token.text))
            .collect();
        assert_eq!(kind_of(&tokens, "name"), Some(Kind::Keyword), "{debug:?}");
        assert!(
            tokens.iter().any(|token| token.kind == Kind::Comment),
            "{debug:?}"
        );
    }

    #[test]
    fn html_tags_and_attributes() {
        let tokens = flat("<div class=\"card\">hi</div>", &HTML);
        assert_eq!(kind_of(&tokens, "div"), Some(Kind::Keyword));
        assert_eq!(kind_of(&tokens, "class"), Some(Kind::Type));
        assert_eq!(kind_of(&tokens, "\"card\""), Some(Kind::Str));
    }

    #[test]
    fn language_lookup_prefers_hint_then_extension() {
        assert_eq!(language_for(Some("rs"), "x.txt").id, "rust");
        assert_eq!(language_for(None, "src/main.py").id, "python");
        assert_eq!(language_for(None, "Makefile").id, "plaintext");
        assert_eq!(language_for(Some("nope"), "a.ts").id, "typescript");
    }
}
