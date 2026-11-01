//! Removal of ANSI escape sequences for comparison purposes.
//!
//! Expected and actual lines are compared with escape sequences removed, so a
//! line that differs only in colouring still matches. The original text is what
//! gets printed; only the comparison sees the stripped form.

/// Strip ANSI escape sequences (CSI and OSC) from `text`.
pub fn strip(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }

        match chars.next() {
            // CSI: ESC '[' ... final byte in @..~
            Some('[') => {
                for c in chars.by_ref() {
                    if ('\x40'..='\x7e').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: ESC ']' ... terminated by BEL or ESC '\'
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\x07' {
                        break;
                    }
                    if c == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            // Any other two-byte escape.
            Some(_) | None => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_colour_sequences() {
        assert_eq!(strip("\x1b[31mhello\x1b[0m"), "hello");
    }

    #[test]
    fn strips_osc_sequences() {
        assert_eq!(strip("\x1b]0;title\x07text"), "text");
    }

    #[test]
    fn leaves_plain_text_unchanged() {
        assert_eq!(strip("plain text"), "plain text");
    }

    #[test]
    fn keeps_embedded_text_between_sequences() {
        assert_eq!(strip("\x1b[1ma\x1b[0m-\x1b[1mb\x1b[0m"), "a-b");
    }
}
