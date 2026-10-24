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
    pub reset: String,
    pub header: String,
    pub added: String,
    pub deleted: String,
    pub line_numbers: String,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            reset: "0".to_string(),
            header: "1".to_string(),
            added: "32".to_string(),
            deleted: "31".to_string(),
            line_numbers: "36".to_string(),
        }
    }
}

/// Why a `--palette` value was rejected.
#[derive(Debug, Eq, PartialEq)]
pub enum PaletteError {
    /// An entry names a capability that does not exist.
    UnrecognizedPrefix(String),
    /// An entry is not of the form `name=value`.
    Unparsable,
}

impl Palette {
    /// Parses a `--palette` value such as `ad=32:de=31`, starting from the
    /// default palette so that only the listed capabilities are overridden.
    pub fn parse(spec: &str) -> Result<Self, PaletteError> {
        let mut palette = Self::default();
        for entry in spec.split(':').filter(|entry| !entry.is_empty()) {
            let Some((name, value)) = entry.split_once('=') else {
                return Err(PaletteError::Unparsable);
            };
            let slot = match name {
                "rs" => &mut palette.reset,
                "hd" => &mut palette.header,
                "ad" => &mut palette.added,
                "de" => &mut palette.deleted,
                "ln" => &mut palette.line_numbers,
                _ => return Err(PaletteError::UnrecognizedPrefix(name.to_string())),
            };
            value.clone_into(slot);
        }
        Ok(palette)
    }
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

fn push_sgr(output: &mut Vec<u8>, parameters: &str) {
    output.extend_from_slice(b"\x1b[");
    output.extend_from_slice(parameters.as_bytes());
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
        assert_eq!(palette.added, "1;34");
        assert_eq!(palette.line_numbers, "35");
        assert_eq!(palette.deleted, Palette::default().deleted);
        assert_eq!(Palette::parse("ad"), Err(PaletteError::Unparsable));
        assert_eq!(
            Palette::parse("xx=1"),
            Err(PaletteError::UnrecognizedPrefix("xx".to_string()))
        );
    }
}
