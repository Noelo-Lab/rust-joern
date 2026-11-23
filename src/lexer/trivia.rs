//! Whitespace, comments and identifier bytes.

pub(super) fn next_nontrivia_byte(bytes: &[u8], at: usize) -> Option<u8> {
    bytes.get(skip_trivia(bytes, at)).copied()
}

pub(super) fn skip_trivia(bytes: &[u8], mut at: usize) -> usize {
    loop {
        while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
        match bytes.get(at..at.saturating_add(2)) {
            Some(b"//") => {
                while bytes.get(at).is_some_and(|b| *b != b'\n') {
                    at += 1;
                }
            }
            Some(b"/*") => {
                at += 2;
                while at + 1 < bytes.len() && &bytes[at..at + 2] != b"*/" {
                    at += 1;
                }
                at = (at + 2).min(bytes.len());
            }
            Some(b"\\\n") => at += 2,
            Some(b"\\\r") if bytes.get(at + 2) == Some(&b'\n') => at += 3,
            _ => return at,
        }
    }
}

pub(super) fn identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' || byte >= 128
}

pub(super) fn identifier_byte(byte: u8) -> bool {
    identifier_start(byte) || byte.is_ascii_digit()
}
