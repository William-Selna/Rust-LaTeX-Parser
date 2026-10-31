//! Mathematical alphanumeric characters (𝐱, ℝ, 𝔉, ...) for math fonts.

// The forward mapping is only needed by the output formats.
#![cfg_attr(not(any(feature = "mathml", feature = "unicode", feature = "svg")), allow(dead_code))]

use crate::ast::MathFontKind;

/// The styled form of an ASCII letter or digit, e.g. `R` in
/// [`MathFontKind::Blackboard`] is `ℝ`. Other characters, and styles with
/// no Unicode equivalent, are returned unchanged.
pub(crate) fn styled_char(kind: MathFontKind, c: char) -> char {
    let (upper, lower, digit) = match kind {
        MathFontKind::Bold => (0x1D400, 0x1D41A, Some(0x1D7CE)),
        MathFontKind::Italic => (0x1D434, 0x1D44E, None),
        MathFontKind::BoldItalic => (0x1D468, 0x1D482, Some(0x1D7CE)),
        MathFontKind::Calligraphic => (0x1D49C, 0x1D4B6, None),
        MathFontKind::Fraktur => (0x1D504, 0x1D51E, None),
        MathFontKind::Blackboard => (0x1D538, 0x1D552, Some(0x1D7D8)),
        MathFontKind::SansSerif => (0x1D5A0, 0x1D5BA, Some(0x1D7E2)),
        MathFontKind::Monospace => (0x1D670, 0x1D68A, Some(0x1D7F6)),
        _ => return c,
    };
    // Letters that were encoded before the Mathematical Alphanumeric
    // Symbols block and so are missing from it.
    let exception = match (kind, c) {
        (MathFontKind::Italic, 'h') => Some('\u{210E}'),
        (MathFontKind::Calligraphic, 'B') => Some('\u{212C}'),
        (MathFontKind::Calligraphic, 'E') => Some('\u{2130}'),
        (MathFontKind::Calligraphic, 'F') => Some('\u{2131}'),
        (MathFontKind::Calligraphic, 'H') => Some('\u{210B}'),
        (MathFontKind::Calligraphic, 'I') => Some('\u{2110}'),
        (MathFontKind::Calligraphic, 'L') => Some('\u{2112}'),
        (MathFontKind::Calligraphic, 'M') => Some('\u{2133}'),
        (MathFontKind::Calligraphic, 'R') => Some('\u{211B}'),
        (MathFontKind::Calligraphic, 'e') => Some('\u{212F}'),
        (MathFontKind::Calligraphic, 'g') => Some('\u{210A}'),
        (MathFontKind::Calligraphic, 'o') => Some('\u{2134}'),
        (MathFontKind::Fraktur, 'C') => Some('\u{212D}'),
        (MathFontKind::Fraktur, 'H') => Some('\u{210C}'),
        (MathFontKind::Fraktur, 'I') => Some('\u{2111}'),
        (MathFontKind::Fraktur, 'R') => Some('\u{211C}'),
        (MathFontKind::Fraktur, 'Z') => Some('\u{2128}'),
        (MathFontKind::Blackboard, 'C') => Some('\u{2102}'),
        (MathFontKind::Blackboard, 'H') => Some('\u{210D}'),
        (MathFontKind::Blackboard, 'N') => Some('\u{2115}'),
        (MathFontKind::Blackboard, 'P') => Some('\u{2119}'),
        (MathFontKind::Blackboard, 'Q') => Some('\u{211A}'),
        (MathFontKind::Blackboard, 'R') => Some('\u{211D}'),
        (MathFontKind::Blackboard, 'Z') => Some('\u{2124}'),
        _ => None,
    };
    if let Some(e) = exception {
        return e;
    }
    let code = match c {
        'A'..='Z' => upper + (c as u32 - 'A' as u32),
        'a'..='z' => lower + (c as u32 - 'a' as u32),
        '0'..='9' => match digit {
            Some(base) => base + (c as u32 - '0' as u32),
            None => return c,
        },
        _ => return c,
    };
    char::from_u32(code).unwrap_or(c)
}

/// The inverse of [`styled_char`]: the font and plain letter for a styled
/// character like `ℝ` or `𝐱`.
pub(crate) fn unstyle_char(c: char) -> Option<(MathFontKind, char)> {
    if c.is_ascii() {
        return None;
    }
    const KINDS: [MathFontKind; 8] = [
        MathFontKind::Bold,
        MathFontKind::Italic,
        MathFontKind::BoldItalic,
        MathFontKind::Calligraphic,
        MathFontKind::Fraktur,
        MathFontKind::Blackboard,
        MathFontKind::SansSerif,
        MathFontKind::Monospace,
    ];
    KINDS.iter().find_map(|&kind| {
        ('A'..='Z')
            .chain('a'..='z')
            .chain('0'..='9')
            .find(|&p| styled_char(kind, p) == c)
            .map(|p| (kind, p))
    })
}

/// Apply [`styled_char`] to every character of a string.
pub(crate) fn styled_str(kind: MathFontKind, s: &str) -> String {
    s.chars().map(|c| styled_char(kind, c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styled_letters() {
        assert_eq!(styled_str(MathFontKind::Blackboard, "RNZ1"), "ℝℕℤ𝟙");
        assert_eq!(styled_str(MathFontKind::Bold, "xA0"), "𝐱𝐀𝟎");
        assert_eq!(styled_str(MathFontKind::Calligraphic, "FL"), "ℱℒ");
        assert_eq!(styled_str(MathFontKind::Fraktur, "gR"), "𝔤ℜ");
        assert_eq!(styled_str(MathFontKind::Italic, "h1"), "ℎ1");
        assert_eq!(styled_str(MathFontKind::Roman, "x"), "x");
        assert_eq!(styled_str(MathFontKind::Bold, "α+"), "α+");
    }
}
