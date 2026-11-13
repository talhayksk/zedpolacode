//! Source preparation shared by every front end.
//!
//! The Zed slash command and the MCP server must agree on what a selection looks
//! like before it reaches the highlighter, and on what a line range means, so
//! both call these functions instead of keeping a private copy.

/// Unifies line endings, expands tabs and drops trailing blank lines.
pub fn normalize(code: &str) -> String {
    let unified = code.replace("\r\n", "\n").replace('\r', "\n");
    let mut expanded = String::with_capacity(unified.len());
    for ch in unified.chars() {
        if ch == '\t' {
            expanded.push_str("    ");
        } else {
            expanded.push(ch);
        }
    }
    expanded.trim_end_matches('\n').to_string()
}

/// Removes the indentation shared by every line so a selection keeps its shape
/// relative to itself.
pub fn dedent(code: &str) -> String {
    let lines: Vec<&str> = code.split('\n').collect();
    let common = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().take_while(|ch| *ch == ' ').count())
        .min()
        .unwrap_or(0);

    if common == 0 {
        return code.to_string();
    }

    lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                line.chars().skip(common).collect::<String>()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Why a range could not be sliced out of a source.
///
/// The variants are deliberately data, not messages: each front end words the
/// problem for its own surface (a slash command versus a tool result).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceError {
    /// The source has no lines at all.
    Empty,
    /// `start` is zero or past the last line.
    StartOutOfRange { start: u32, total: u32 },
    /// `end` comes before `start`.
    Reversed { start: u32, end: u32 },
}

/// The lines a range selected, plus the line number they start at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slice {
    pub code: String,
    /// 1-based number of the first selected line.
    pub first_line: u32,
}

/// Selects lines `start..=end` (1-based, inclusive) out of `raw`.
///
/// `end == 0` means "to the end of the source", which is also how the whole
/// file is requested (`slice_lines(raw, 1, 0)`). `end` past the last line is
/// clamped instead of being an error, because a range typed from a stale view
/// of the file should still render.
pub fn slice_lines(raw: &str, start: u32, end: u32) -> Result<Slice, SliceError> {
    let all: Vec<&str> = raw.lines().collect();
    if all.is_empty() {
        return Err(SliceError::Empty);
    }
    let total = all.len() as u32;

    // `start == 0` is not a line; treating it as out of range keeps the
    // subtraction below total.
    if start == 0 || start > total {
        return Err(SliceError::StartOutOfRange { start, total });
    }

    let end = if end == 0 { total } else { end.min(total) };
    if end < start {
        return Err(SliceError::Reversed { start, end });
    }

    Ok(Slice {
        code: all[(start as usize - 1)..(end as usize)].join("\n"),
        first_line: start,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedents_blocks() {
        let code = "    fn main() {\n        println!(\"hi\");\n    }";
        assert_eq!(dedent(code), "fn main() {\n    println!(\"hi\");\n}");
    }

    #[test]
    fn keeps_relative_indentation_when_no_common_prefix() {
        let code = "fn a() {}\n    fn b() {}";
        assert_eq!(dedent(code), code);
    }

    #[test]
    fn normalizes_newlines_and_tabs() {
        assert_eq!(normalize("a\tb\r\nc\r\n"), "a    b\nc");
    }

    #[test]
    fn slices_a_single_line() {
        let slice = slice_lines("one\ntwo\nthree", 2, 2).unwrap();
        assert_eq!(slice.code, "two");
        assert_eq!(slice.first_line, 2);
    }

    #[test]
    fn slices_to_the_end_of_the_file() {
        let slice = slice_lines("one\ntwo\nthree", 2, 0).unwrap();
        assert_eq!(slice.code, "two\nthree");
        assert_eq!(slice.first_line, 2);
    }

    #[test]
    fn slices_the_whole_file() {
        let slice = slice_lines("one\ntwo\nthree", 1, 0).unwrap();
        assert_eq!(slice.code, "one\ntwo\nthree");
        assert_eq!(slice.first_line, 1);
    }

    #[test]
    fn clamps_an_end_past_the_last_line() {
        let slice = slice_lines("one\ntwo", 2, 99).unwrap();
        assert_eq!(slice.code, "two");
        assert_eq!(slice.first_line, 2);
    }

    #[test]
    fn rejects_a_start_past_the_last_line() {
        assert_eq!(
            slice_lines("one\ntwo", 3, 0),
            Err(SliceError::StartOutOfRange { start: 3, total: 2 })
        );
    }

    #[test]
    fn rejects_zero_as_the_start_line() {
        assert_eq!(
            slice_lines("one\ntwo", 0, 1),
            Err(SliceError::StartOutOfRange { start: 0, total: 2 })
        );
    }

    #[test]
    fn rejects_a_reversed_range() {
        assert_eq!(
            slice_lines("one\ntwo\nthree", 3, 1),
            Err(SliceError::Reversed { start: 3, end: 1 })
        );
    }

    #[test]
    fn rejects_an_empty_source() {
        assert_eq!(slice_lines("", 1, 0), Err(SliceError::Empty));
    }
}
