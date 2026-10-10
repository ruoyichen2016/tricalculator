//! Reading PCGen data files, which are not reliably UTF-8.
//!
//! The published third-party packs are largely Windows-1252. A `.lst` file with
//! a curly apostrophe (`0x92`) or an accented letter (`0xE9`) is not valid UTF-8,
//! and reading it strictly aborted the whole pack: 20 of the 196 top-level
//! third-party campaigns failed to import for this reason alone, including
//! *Book of Exalted Deeds*, *Frostburn* and *Races of the Wild*.
//!
//! PCGen itself tolerates this, so the reader does too: try UTF-8 first, and
//! fall back to Windows-1252. Every byte maps to a code point in CP1252, so the
//! fallback cannot fail.

use std::fs;
use std::io;
use std::path::Path;

/// Read a text file, falling back to Windows-1252 when it is not valid UTF-8.
pub fn read_text(path: impl AsRef<Path>) -> io::Result<String> {
    let bytes = fs::read(path)?;
    Ok(match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => decode_cp1252(error.as_bytes()),
    })
}

/// Decode bytes as Windows-1252.
///
/// `0x00..=0x7F` and `0xA0..=0xFF` agree with Unicode; only the `0x80..=0x9F`
/// range differs, and five of those are undefined (they fall through to the
/// matching C1 control, as the WHATWG mapping does).
fn decode_cp1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| match byte {
            0x80 => '\u{20AC}', // euro
            0x82 => '\u{201A}', // single low quotation mark
            0x83 => '\u{0192}', // f with hook
            0x84 => '\u{201E}', // double low quotation mark
            0x85 => '\u{2026}', // horizontal ellipsis
            0x86 => '\u{2020}', // dagger
            0x87 => '\u{2021}', // double dagger
            0x88 => '\u{02C6}', // modifier circumflex
            0x89 => '\u{2030}', // per mille
            0x8A => '\u{0160}', // S with caron
            0x8B => '\u{2039}', // single left angle quotation mark
            0x8C => '\u{0152}', // OE ligature
            0x8E => '\u{017D}', // Z with caron
            0x91 => '\u{2018}', // left single quotation mark
            0x92 => '\u{2019}', // right single quotation mark
            0x93 => '\u{201C}', // left double quotation mark
            0x94 => '\u{201D}', // right double quotation mark
            0x95 => '\u{2022}', // bullet
            0x96 => '\u{2013}', // en dash
            0x97 => '\u{2014}', // em dash
            0x98 => '\u{02DC}', // small tilde
            0x99 => '\u{2122}', // trade mark
            0x9A => '\u{0161}', // s with caron
            0x9B => '\u{203A}', // single right angle quotation mark
            0x9C => '\u{0153}', // oe ligature
            0x9E => '\u{017E}', // z with caron
            0x9F => '\u{0178}', // Y with diaeresis
            other => char::from_u32(u32::from(other)).unwrap_or('\u{FFFD}'),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_utf8_is_kept_as_is() {
        let bytes = "Brontë".as_bytes().to_vec();
        assert_eq!(String::from_utf8(bytes).unwrap(), "Brontë");
    }

    #[test]
    fn cp1252_high_bytes_decode_to_the_intended_characters() {
        // 0x92 is a right single quotation mark, 0xE9 is e-acute. Neither is
        // valid UTF-8 on its own.
        assert_eq!(decode_cp1252(&[0x92]), "\u{2019}");
        assert_eq!(decode_cp1252(&[0xE9]), "é");
        assert_eq!(
            decode_cp1252(&[b'Z', b'a', b'r', 0x92, b'a', b'n']),
            "Zar\u{2019}an",
            "a curly apostrophe must survive"
        );
        assert_eq!(decode_cp1252(&[b'T', 0xEB, b'r']), "Tër");
    }

    #[test]
    fn ascii_and_undefined_bytes_are_not_lost() {
        assert_eq!(decode_cp1252(b"plain ascii"), "plain ascii");
        // 0x81 is undefined in CP1252; it must still round-trip as a character
        // rather than panic or vanish.
        assert_eq!(decode_cp1252(&[0x81]).chars().count(), 1);
    }
}
