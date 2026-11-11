// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

//! Colorizing of the normal, unified and context output formats
//! (`--color` and `--palette`).

use crate::params::Format;

/// When to colorize the output (`--color[=WHEN]`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColorWhen {
    #[default]
    Never,
    Always,
    /// Only when standard output is a terminal.
    Auto,
}

/// SGR parameters used for each kind of output line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Palette {
    pub reset: Vec<u8>,
    pub header: Vec<u8>,
    pub added: Vec<u8>,
    pub deleted: Vec<u8>,
    pub line_numbers: Vec<u8>,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            reset: b"0".to_vec(),
            header: b"1".to_vec(),
            added: b"32".to_vec(),
            deleted: b"31".to_vec(),
            line_numbers: b"36".to_vec(),
        }
    }
}

/// Why a `--palette` value was rejected.
#[derive(Debug, Eq, PartialEq)]
pub enum PaletteError {
    /// An entry names a capability that does not exist, or its value has a
    /// malformed escape.
    UnrecognizedPrefix(String),
    /// An entry is not of the form `name=value`.
    Unparsable,
}

impl Palette {
    /// Parses a `--palette` value such as `ad=32:de=31`, starting from the
    /// default palette so that only the listed capabilities are overridden.
    /// Values are written as for `dircolors`, see [`decode_value`].
    pub fn parse(spec: &str) -> Result<Self, PaletteError> {
        let mut palette = Self::default();
        let mut rest = spec.trim_start_matches(':');
        while !rest.is_empty() {
            let name_end = rest
                .find(['=', ':'])
                .filter(|&end| rest[end..].starts_with('='));
            let Some(name_end) = name_end else {
                return Err(PaletteError::Unparsable);
            };
            let name = &rest[..name_end];
            let slot = match name {
                "rs" => &mut palette.reset,
                "hd" => &mut palette.header,
                "ad" => &mut palette.added,
                "de" => &mut palette.deleted,
                "ln" => &mut palette.line_numbers,
                _ => return Err(PaletteError::UnrecognizedPrefix(name.to_string())),
            };
            let encoded = &rest[name_end + 1..];
            let Some((value, length)) = decode_value(encoded.as_bytes()) else {
                return Err(PaletteError::UnrecognizedPrefix(name.to_string()));
            };
            *slot = value;
            rest = encoded[length..].trim_start_matches(':');
        }
        Ok(palette)
    }
}

/// Decodes the value at the start of `input`, up to an unescaped colon or
/// the end, and returns it with the number of bytes it took.
///
/// A backslash introduces an octal (`\033`) or hexadecimal (`\x1b`) byte, of
/// which the low eight bits are kept, one of the escapes `\a`, `\b`, `\e`,
/// `\f`, `\n`, `\r`, `\t`, `\v`, `\?` (delete) and `\_` (space), or
/// stands for the next byte itself. A caret makes a control character of the
/// next byte: `^[` is escape and `^?` is delete.
///
/// Returns `None` when the value ends inside an escape or a caret is followed
/// by anything else than `?` or a byte from `@` to `~`.
fn decode_value(input: &[u8]) -> Option<(Vec<u8>, usize)> {
    let mut value = Vec::new();
    let mut position = 0;
    while let Some(&byte) = input.get(position).filter(|&&byte| byte != b':') {
        position += 1;
        match byte {
            b'\\' => {
                let escaped = *input.get(position)?;
                if matches!(escaped, b'0'..=b'7') {
                    value.push(take_number(input, &mut position, 8));
                    continue;
                }
                position += 1;
                value.push(match escaped {
                    b'x' | b'X' => take_number(input, &mut position, 16),
                    b'a' => 0x07,
                    b'b' => 0x08,
                    b'e' => 0x1b,
                    b'f' => 0x0c,
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    b'v' => 0x0b,
                    b'?' => 0x7f,
                    b'_' => b' ',
                    other => other,
                });
            }
            b'^' => {
                let control = *input.get(position)?;
                position += 1;
                value.push(match control {
                    b'@'..=b'~' => control & 0x1f,
                    b'?' => 0x7f,
                    _ => return None,
                });
            }
            _ => value.push(byte),
        }
    }
    Some((value, position))
}

/// Reads the digits at `position` as a number in the given radix, moving
/// `position` behind them. Only the low eight bits of the number are kept,
/// and no digit at all makes zero.
fn take_number(input: &[u8], position: &mut usize, radix: u8) -> u8 {
    let mut number: u8 = 0;
    while let Some(digit) = input
        .get(*position)
        .and_then(|&byte| char::from(byte).to_digit(radix.into()))
    {
        // A digit is below the radix, which fits in a byte.
        number = number.wrapping_mul(radix).wrapping_add(digit as u8);
        *position += 1;
    }
    number
}

/// The role of a line of diff output, which decides its color.
#[derive(Clone, Copy)]
enum LineKind {
    Plain,
    Header,
    Added,
    Deleted,
    LineNumbers,
}

/// Rewrites the output of a diff in the given format, wrapping each line in
/// the escape sequences of the palette entry matching its role.
///
/// Each kind of line is recognizable from its prefix alone, because the text
/// of the compared files always appears behind a marker such as `< `, `+` or
/// `! `. In the context format, `! ` lines are deletions until the `--- `
/// range line of the new file is reached, and additions after it.
#[must_use]
pub fn colorize(output: &[u8], format: Format, palette: &Palette) -> Vec<u8> {
    let mut colored = Vec::with_capacity(output.len() + output.len() / 4);
    let mut in_new_file_part = false;
    let lines = output.split_inclusive(|&byte| byte == b'\n');
    for (index, line) in lines.enumerate() {
        let kind = match format {
            Format::Normal => match line.first() {
                Some(b'<') => LineKind::Deleted,
                Some(b'>') => LineKind::Added,
                Some(b'0'..=b'9') => LineKind::LineNumbers,
                _ => LineKind::Plain,
            },
            Format::Unified => match line.first() {
                _ if index < 2 => LineKind::Header,
                Some(b'-') => LineKind::Deleted,
                Some(b'+') => LineKind::Added,
                Some(b'@') => LineKind::LineNumbers,
                _ => LineKind::Plain,
            },
            Format::Context => {
                if index < 2 {
                    LineKind::Header
                } else if line.starts_with(b"*** ") {
                    in_new_file_part = false;
                    LineKind::LineNumbers
                } else if line.starts_with(b"--- ") {
                    in_new_file_part = true;
                    LineKind::LineNumbers
                } else if line.starts_with(b"- ") {
                    LineKind::Deleted
                } else if line.starts_with(b"+ ") {
                    LineKind::Added
                } else if line.starts_with(b"! ") {
                    if in_new_file_part {
                        LineKind::Added
                    } else {
                        LineKind::Deleted
                    }
                } else {
                    LineKind::Plain
                }
            }
            Format::Ed | Format::SideBySide => LineKind::Plain,
        };
        let sgr = match kind {
            LineKind::Plain => {
                colored.extend_from_slice(line);
                continue;
            }
            LineKind::Header => &palette.header,
            LineKind::Added => &palette.added,
            LineKind::Deleted => &palette.deleted,
            LineKind::LineNumbers => &palette.line_numbers,
        };
        let (text, newline) = match line.strip_suffix(b"\n") {
            Some(text) => (text, &b"\n"[..]),
            None => (line, &b""[..]),
        };
        push_sgr(&mut colored, sgr);
        colored.extend_from_slice(text);
        push_sgr(&mut colored, &palette.reset);
        colored.extend_from_slice(newline);
    }
    colored
}

fn push_sgr(output: &mut Vec<u8>, parameters: &[u8]) {
    output.extend_from_slice(b"\x1b[");
    output.extend_from_slice(parameters);
    output.push(b'm');
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn colorize_str(output: &str, format: Format) -> String {
        String::from_utf8(colorize(output.as_bytes(), format, &Palette::default())).unwrap()
    }

    #[test]
    fn normal() {
        assert_eq!(
            colorize_str("3,4c3\n< owl\n< elk\n---\n> yak\n", Format::Normal),
            "\x1b[36m3,4c3\x1b[0m\n\x1b[31m< owl\x1b[0m\n\x1b[31m< elk\x1b[0m\n---\n\x1b[32m> yak\x1b[0m\n"
        );
    }

    #[test]
    fn unified() {
        // The deleted text looks like a header line, but only the first two
        // lines are headers.
        let output = "--- left\n+++ right\n@@ -2 +2 @@\n--- not a header\n+++ nor this\n";
        assert_eq!(
            colorize_str(output, Format::Unified),
            "\x1b[1m--- left\x1b[0m\n\x1b[1m+++ right\x1b[0m\n\x1b[36m@@ -2 +2 @@\x1b[0m\n\
             \x1b[31m--- not a header\x1b[0m\n\x1b[32m+++ nor this\x1b[0m\n"
        );
    }

    #[test]
    fn context() {
        let output = "*** left\n--- right\n***************\n*** 1,2 ****\n  cat\n! dog\n\
                      --- 1,3 ----\n  cat\n! cow\n+ pig\n";
        assert_eq!(
            colorize_str(output, Format::Context),
            "\x1b[1m*** left\x1b[0m\n\x1b[1m--- right\x1b[0m\n***************\n\
             \x1b[36m*** 1,2 ****\x1b[0m\n  cat\n\x1b[31m! dog\x1b[0m\n\
             \x1b[36m--- 1,3 ----\x1b[0m\n  cat\n\x1b[32m! cow\x1b[0m\n\x1b[32m+ pig\x1b[0m\n"
        );
    }

    #[test]
    fn missing_newline_marker_is_plain() {
        assert_eq!(
            colorize_str("1c1\n< ant\n---\n> bee\n\\ No newline at end of file\n", Format::Normal),
            "\x1b[36m1c1\x1b[0m\n\x1b[31m< ant\x1b[0m\n---\n\x1b[32m> bee\x1b[0m\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn palette() {
        let palette = Palette::parse("ad=1;34:ln=35").unwrap();
        assert_eq!(palette.added, b"1;34");
        assert_eq!(palette.line_numbers, b"35");
        assert_eq!(palette.deleted, Palette::default().deleted);
        assert_eq!(Palette::parse("ad"), Err(PaletteError::Unparsable));
        assert_eq!(
            Palette::parse("xx=1"),
            Err(PaletteError::UnrecognizedPrefix("xx".to_string()))
        );
    }

    #[test]
    fn palette_value_escapes() {
        let added = |spec: &str| Palette::parse(spec).map(|palette| palette.added);
        assert_eq!(added(r"ad=\061;3\62"), Ok(b"1;32".to_vec()));
        assert_eq!(added(r"ad=\x31;3\X32"), Ok(b"1;32".to_vec()));
        // Any number of digits is read, and the low byte is kept.
        assert_eq!(added(r"ad=\0611\400\x3132"), Ok(b"\x89\x002".to_vec()));
        assert_eq!(added(r"ad=\08\x\xg"), Ok(b"\x008\x00\x00g".to_vec()));
        assert_eq!(
            added(r"ad=\a\b\e\f\n\r\t\v\?\_"),
            Ok(b"\x07\x08\x1b\x0c\n\r\t\x0b\x7f ".to_vec())
        );
        assert_eq!(added(r"ad=\\\:\=\^\q\E"), Ok(br"\:=^qE".to_vec()));
        assert_eq!(
            added("ad=^@^[^{^a^A^~^_^`^?"),
            Ok(b"\x00\x1b\x1b\x01\x01\x1e\x1f\x00\x7f".to_vec())
        );
        assert_eq!(added("ad=1=2"), Ok(b"1=2".to_vec()));
        assert_eq!(added("ad=é"), Ok("é".as_bytes().to_vec()));

        let palette = Palette::parse(r":ad=3\:2::de=^[:").unwrap();
        assert_eq!(palette.added, b"3:2");
        assert_eq!(palette.deleted, b"\x1b");

        for spec in [r"ad=\", "ad=^", "ad=^1", "ad=^ ", "ad=^:de=4", "ad=^é"] {
            assert_eq!(
                Palette::parse(spec),
                Err(PaletteError::UnrecognizedPrefix("ad".to_string())),
                "{spec}"
            );
        }
    }
}
