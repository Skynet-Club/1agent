pub fn contains_disallowed_control(text: &str) -> bool {
    text.chars().any(is_disallowed_control)
}

pub fn is_disallowed_control(character: char) -> bool {
    matches!(
        character,
        '\u{0000}'..='\u{0008}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{000E}'..='\u{001F}'
            | '\u{007F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
            | '\u{E0000}'..='\u{E007F}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_whitespace_is_allowed() {
        assert!(!contains_disallowed_control("line 1\nline 2\tvalue\r\n"));
    }

    #[test]
    fn bidi_zero_width_and_tag_controls_are_rejected() {
        for value in ["a\u{202E}b", "a\u{200B}b", "a\u{E0041}b", "a\u{0000}b"] {
            assert!(contains_disallowed_control(value), "{value:?}");
        }
    }
}
