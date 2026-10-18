//! Bytes to text, for the character sets mail still arrives in: `mailcraft-charset`'s decoder,
//! which vCard files are read with too, and line endings.

pub(crate) use mailcraft_charset::{decode, guess};

/// Line endings as `\n`: CRLF and a lone CR both become LF.
pub(crate) fn unix_lines(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_string();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_endings() {
        assert_eq!(unix_lines("a\r\nb\rc\nd\r\n"), "a\nb\nc\nd\n");
        assert_eq!(unix_lines("\r\r\n\r"), "\n\n\n");
        assert_eq!(unix_lines("none"), "none");
    }
}
