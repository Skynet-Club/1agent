//! Pure text helpers for the terminal presentation adapter.
//!
//! These functions are independent of TUI state so they can be tested and
//! reused without constructing a terminal.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::safety::is_disallowed_control;

pub fn sanitize_inline(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character == '\n' || character == '\t' || is_disallowed_control(character) {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn sanitize_multiline(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\n' => output.push('\n'),
            '\t' => output.push_str("    "),
            '\r' => output.push('�'),
            character if is_disallowed_control(character) => output.push('�'),
            character => output.push(character),
        }
    }
    output
}

pub fn sanitize_input(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character == '\n'
                || character == '\r'
                || character == '\t'
                || is_disallowed_control(character)
            {
                ' '
            } else {
                character
            }
        })
        .collect()
}

pub fn fit_row(text: &str, columns: usize) -> String {
    let text = truncate_to_width(text, columns);
    let padding = columns.saturating_sub(text.width());
    format!("{text}{}", " ".repeat(padding))
}

pub fn wrap_display(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut output = Vec::new();
    for logical_line in text.split('\n') {
        if logical_line.is_empty() {
            output.push(String::new());
            continue;
        }
        let mut line = String::new();
        let mut line_width = 0;
        for character in logical_line.chars() {
            let character_width = character.width().unwrap_or(0);
            if line_width > 0 && line_width + character_width > width {
                output.push(line);
                line = String::new();
                line_width = 0;
            }
            line.push(character);
            line_width += character_width;
        }
        output.push(line);
    }
    output
}

pub fn split_display_width(text: &str, limit: usize) -> (&str, &str) {
    let mut width = 0;
    for (index, grapheme) in text.grapheme_indices(true) {
        let next = width + grapheme.width();
        if next > limit {
            return (&text[..index], &text[index..]);
        }
        width = next;
    }
    (text, "")
}

pub fn truncate_to_width(text: &str, limit: usize) -> String {
    if text.width() <= limit {
        return text.to_string();
    }
    if limit == 0 {
        return String::new();
    }
    let mut output = String::new();
    let mut width = 0;
    for character in text.chars() {
        let character_width = character.width().unwrap_or(0);
        if width + character_width >= limit {
            break;
        }
        output.push(character);
        width += character_width;
    }
    output.push('…');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_inline_flattens_controls_and_tabs() {
        assert_eq!(sanitize_inline("a\tb\nc\u{7}d  "), "a b c d");
        assert_eq!(sanitize_inline("  padded  "), "padded");
    }

    #[test]
    fn sanitize_multiline_keeps_newlines_but_replaces_controls() {
        assert_eq!(sanitize_multiline("a\nb\tc\rd\u{7}"), "a\nb    c�d�");
        assert_eq!(sanitize_input("a\nb\tc"), "a b c");
    }

    #[test]
    fn wrap_display_wraps_and_respects_wide_characters() {
        assert_eq!(wrap_display("abcdef", 3), ["abc", "def"]);
        assert_eq!(wrap_display("a\n\nb", 3), ["a", "", "b"]);
        assert_eq!(wrap_display("你好世界", 4), ["你好", "世界"]);
    }

    #[test]
    fn fit_row_pads_and_truncates() {
        assert_eq!(fit_row("ab", 4), "ab  ");
        assert_eq!(fit_row("abcdef", 4), "abc…");
        assert_eq!(fit_row("ab", 2), "ab");
    }

    #[test]
    fn split_and_truncate_use_display_width() {
        assert_eq!(split_display_width("你好世界", 4), ("你好", "世界"));
        assert_eq!(truncate_to_width("你好世界", 4), "你…");
    }
}
