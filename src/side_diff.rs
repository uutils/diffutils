// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use core::cmp::{max, min};
use diff::Result;
use std::{io::Write, vec};
use unicode_width::UnicodeWidthStr;

use crate::params::Params;

const GUTTER_WIDTH_MIN: usize = 3;

struct CharIter<'a> {
    current: &'a [u8],
}

struct Config {
    sdiff_half_width: usize,
    sdiff_column_two_offset: usize,
    tab_size: usize,
    expanded: bool,
    separator_pos: usize,
}

impl<'a> From<&'a [u8]> for CharIter<'a> {
    fn from(value: &'a [u8]) -> Self {
        CharIter { current: value }
    }
}

impl<'a> Iterator for CharIter<'a> {
    // (bytes for the next char, visible width)
    type Item = (&'a [u8], usize);

    fn next(&mut self) -> Option<Self::Item> {
        let max = self.current.len().min(4);

        // We reached the end.
        if max == 0 {
            return None;
        }

        // Try to find the next utf-8 character, if present in the next 4 bytes.
        let mut index = 1;
        let mut view = &self.current[..index];
        let mut char = str::from_utf8(view);
        while char.is_err() {
            index += 1;
            if index > max {
                break;
            }
            view = &self.current[..index];
            char = str::from_utf8(view)
        }

        match char {
            Ok(c) => {
                self.current = self
                    .current
                    .get(view.len()..)
                    .unwrap_or(&self.current[0..0]);
                Some((view, UnicodeWidthStr::width(c)))
            }
            Err(_) => {
                // We did not find an utf-8 char within the next 4 bytes, return the single byte.
                self.current = &self.current[1..];
                Some((&view[..1], 1))
            }
        }
    }
}

impl Config {
    pub fn new(full_width: usize, tab_size: usize, expanded: bool) -> Self {
        // diff uses this calculation to calculate the size of a half line
        // based on the options passed (like -w, -t, etc.). It's actually
        // pretty useless, because we (actually) don't have any size modifiers
        // that can change this, however I just want to leave the calculate
        // here, since it's not very clear and may cause some confusion

        let w = full_width as isize;
        let t = tab_size as isize;
        let t_plus_g = t + GUTTER_WIDTH_MIN as isize;
        let unaligned_off = (w >> 1) + (t_plus_g >> 1) + (w & t_plus_g & 1);
        let off = unaligned_off - unaligned_off % t;
        let hw = max(0, min(off - GUTTER_WIDTH_MIN as isize, w - off)) as usize;
        let c2o = if hw != 0 { off as usize } else { w as usize };

        Self {
            expanded,
            sdiff_column_two_offset: c2o,
            tab_size,
            sdiff_half_width: hw,
            separator_pos: ((hw + c2o - 1) >> 1),
        }
    }
}

fn format_tabs_and_spaces<T: Write>(
    from: usize,
    to: usize,
    config: &Config,
    buf: &mut T,
) -> std::io::Result<()> {
    let expanded = config.expanded;
    let tab_size = config.tab_size;
    let mut current = from;

    if current > to {
        return Ok(());
    }

    if expanded {
        while current < to {
            buf.write_all(b" ")?;
            current += 1;
        }
        return Ok(());
    }

    while current + (tab_size - current % tab_size) <= to {
        let next_tab = current + (tab_size - current % tab_size);
        buf.write_all(b"\t")?;
        current = next_tab;
    }

    while current < to {
        buf.write_all(b" ")?;
        current += 1;
    }

    Ok(())
}

fn process_half_line<T: Write>(
    s: &[u8],
    max_width: usize,
    is_right: bool,
    white_space_gutter: bool,
    config: &Config,
    buf: &mut T,
) -> std::io::Result<()> {
    if s.is_empty() {
        if !is_right {
            format_tabs_and_spaces(
                0,
                max_width
                    + if white_space_gutter {
                        GUTTER_WIDTH_MIN
                    } else {
                        1
                    },
                config,
                buf,
            )?;
        }

        return Ok(());
    }

    if max_width > config.sdiff_half_width {
        return Ok(());
    }

    if max_width > config.sdiff_column_two_offset && !is_right {
        return Ok(());
    }

    let expanded = config.expanded;
    let tab_size = config.tab_size;
    let sdiff_column_two_offset = config.sdiff_column_two_offset;
    let mut current_width = 0;
    let iter = CharIter::from(s);

    // the encoding will probably be compatible with utf8, so we can take advantage
    // of that to get the size of the columns and iterate without breaking the encoding of anything.
    // It seems like a good trade, since there is still a fallback in case it is not utf8.
    // But I think it would be better if we used some lib that would allow us to handle this
    // in the best way possible, in order to avoid overhead (currently 2 for loops are needed).
    // There is a library called mcel (mcel.h) that is used in GNU diff, but the documentation
    // about it is very scarce, nor is its use documented on the internet. In fact, from my
    // research I didn't even find any information about it in the GNU lib's own documentation.

    for c in iter {
        let (char, c_width) = c;

        if current_width + c_width > max_width {
            break;
        }

        match char {
            b"\t" => {
                if expanded && (current_width + tab_size - (current_width % tab_size)) <= max_width
                {
                    let mut spaces = tab_size - (current_width % tab_size);
                    while spaces > 0 {
                        buf.write_all(b" ")?;
                        current_width += 1;
                        spaces -= 1;
                    }
                } else if current_width + tab_size - (current_width % tab_size) <= max_width {
                    buf.write_all(b"\t")?;
                    current_width += tab_size - (current_width % tab_size);
                }
            }
            b"\n" => {
                break;
            }
            b"\r" => {
                buf.write_all(b"\r")?;
                format_tabs_and_spaces(0, sdiff_column_two_offset, config, buf)?;
                current_width = 0;
            }
            b"\0" | b"\x07" | b"\x0C" | b"\x0B" => {
                buf.write_all(char)?;
            }
            _ => {
                buf.write_all(char)?;
                current_width += c_width;
            }
        }
    }

    // gnu sdiff do not tabulate the hole empty right line, instead, just keep the line empty
    if !is_right {
        // we always sum + 1 or + GUTTER_WIDTH_MIN cause we want to expand
        // up to the third column of the gutter column if the gutter is gutter white space,
        // otherwise we can expand to only the first column of the gutter middle column, cause
        // the next is the sep char
        format_tabs_and_spaces(
            current_width,
            max_width
                + if white_space_gutter {
                    GUTTER_WIDTH_MIN
                } else {
                    1
                },
            config,
            buf,
        )?;
    }

    Ok(())
}

fn push_output<T: Write>(
    left_ln: &[u8],
    right_ln: &[u8],
    symbol: u8,
    output: &mut T,
    config: &Config,
) -> std::io::Result<()> {
    if left_ln.is_empty() && right_ln.is_empty() {
        writeln!(output)?;
        return Ok(());
    }

    let white_space_gutter = symbol == b' ';
    let half_width = config.sdiff_half_width;
    let column_two_offset = config.sdiff_column_two_offset;
    let separator_pos = config.separator_pos;
    let put_new_line = true; // should be false when | is allowed

    // this involves a lot of the '|' mark, however, as it is not active,
    // it is better to deactivate it as it introduces visual bug if
    // the line is empty.
    // if !left_ln.is_empty() {
    //     put_new_line = put_new_line || (left_ln.last() == Some(&b'\n'));
    // }
    // if !right_ln.is_empty() {
    //     put_new_line = put_new_line || (right_ln.last() == Some(&b'\n'));
    // }

    process_half_line(
        left_ln,
        half_width,
        false,
        white_space_gutter,
        config,
        output,
    )?;
    if symbol != b' ' {
        // the diff always want to put all tabs possible in the usable are,
        // even in the middle space between the gutters if possible.

        output.write_all(&[symbol])?;
        if !right_ln.is_empty() {
            format_tabs_and_spaces(separator_pos + 1, column_two_offset, config, output)?;
        }
    }
    process_half_line(
        right_ln,
        half_width,
        true,
        white_space_gutter,
        config,
        output,
    )?;

    if put_new_line {
        writeln!(output)?;
    }

    Ok(())
}

pub fn diff<T: Write>(
    from_file: &[u8],
    to_file: &[u8],
    output: &mut T,
    params: &Params,
) -> Vec<u8> {
    //      ^ The left file  ^ The right file

    let mut left_lines: Vec<&[u8]> = from_file.split_inclusive(|&c| c == b'\n').collect();
    let mut right_lines: Vec<&[u8]> = to_file.split_inclusive(|&c| c == b'\n').collect();
    let config = Config::new(params.width, params.tabsize, params.expand_tabs);

    if left_lines.last() == Some(&&b""[..]) {
        left_lines.pop();
    }

    if right_lines.last() == Some(&&b""[..]) {
        right_lines.pop();
    }

    /*
    DISCLAIMER:
    Currently the diff engine does not produce results like the diff engine used in GNU diff,
    so some results may be inaccurate. For example, the line difference marker "|", according
    to the GNU documentation, appears when the same lines (only the actual line, although the
    relative line may change the result, so occasionally '|' markers appear with the same lines)
    are different but exist in both files. In the current solution the same result cannot be
    obtained because the diff engine does not return Both if both exist but are different,
    but instead returns a Left and a Right for each one, implying that two lines were added
    and deleted. Furthermore, the GNU diff program apparently stores some internal state
    (this internal state is just a note about how the diff engine works) about the lines.
    For example, an added or removed line directly counts in the line query of the original
    lines to be printed in the output. Because of this imbalance caused by additions and
    deletions, the characters ( and ) are introduced. They basically represent lines without
    context, which have lost their pair in the other file due to additions or deletions. Anyway,
    my goal with this disclaimer is to warn that for some reason, whether it's the diff engine's
    inability to determine and predict/precalculate the result of GNU's sdiff, with this software it's
    not possible to reproduce results that are 100% faithful to GNU's, however, the basic premise
    e of side diff of showing added and removed lines and creating edit scripts is totally possible.
    More studies are needed to cover GNU diff side by side with 100% accuracy, which is one of
    the goals of this project : )
    */
    for result in diff::slice(&left_lines, &right_lines) {
        match result {
            Result::Left(left_ln) => push_output(left_ln, b"", b'<', output, &config).unwrap(),
            Result::Right(right_ln) => push_output(b"", right_ln, b'>', output, &config).unwrap(),
            Result::Both(left_ln, right_ln) => {
                push_output(left_ln, right_ln, b' ', output, &config).unwrap()
            }
        }
    }

    vec![]
}

#[cfg(test)]
mod tests {
    const DEF_TAB_SIZE: usize = 4;

    use super::*;

    mod format_tabs_and_spaces {
        use super::*;

        const CONFIG_E_T: Config = Config {
            sdiff_half_width: 60,
            tab_size: DEF_TAB_SIZE,
            expanded: true,
            sdiff_column_two_offset: 0,
            separator_pos: 0,
        };

        const CONFIG_E_F: Config = Config {
            sdiff_half_width: 60,
            tab_size: DEF_TAB_SIZE,
            expanded: false,
            sdiff_column_two_offset: 0,
            separator_pos: 0,
        };

        #[test]
        fn test_format_tabs_and_spaces_expanded_false() {
            let mut buf = vec![];
            format_tabs_and_spaces(0, 5, &CONFIG_E_F, &mut buf).unwrap();
            assert_eq!(buf, vec![b'\t', b' ']);
        }

        #[test]
        fn test_format_tabs_and_spaces_expanded_true() {
            let mut buf = vec![];
            format_tabs_and_spaces(0, 5, &CONFIG_E_T, &mut buf).unwrap();
            assert_eq!(buf, vec![b' '; 5]);
        }

        #[test]
        fn test_format_tabs_and_spaces_from_greater_than_to() {
            let mut buf = vec![];
            format_tabs_and_spaces(6, 5, &CONFIG_E_F, &mut buf).unwrap();
            assert!(buf.is_empty());
        }

        #[test]
        fn test_format_from_non_zero_position() {
            let mut buf = vec![];
            format_tabs_and_spaces(2, 7, &CONFIG_E_F, &mut buf).unwrap();
            assert_eq!(buf, vec![b'\t', b' ', b' ', b' ']);
        }

        #[test]
        fn test_multiple_full_tabs_needed() {
            let mut buf = vec![];
            format_tabs_and_spaces(0, 12, &CONFIG_E_F, &mut buf).unwrap();
            assert_eq!(buf, vec![b'\t', b'\t', b'\t']);
        }

        #[test]
        fn test_uneven_tab_boundary_with_spaces() {
            let mut buf = vec![];
            format_tabs_and_spaces(3, 10, &CONFIG_E_F, &mut buf).unwrap();
            assert_eq!(buf, vec![b'\t', b'\t', b' ', b' ']);
        }

        #[test]
        fn test_expanded_true_with_offset() {
            let mut buf = vec![];
            format_tabs_and_spaces(3, 9, &CONFIG_E_T, &mut buf).unwrap();
            assert_eq!(buf, vec![b' '; 6]);
        }

        #[test]
        fn test_exact_tab_boundary_from_midpoint() {
            let mut buf = vec![];
            format_tabs_and_spaces(4, 8, &CONFIG_E_F, &mut buf).unwrap();
            assert_eq!(buf, vec![b'\t']);
        }

        #[test]
        fn test_mixed_tabs_and_spaces_edge_case() {
            let mut buf = vec![];
            format_tabs_and_spaces(5, 9, &CONFIG_E_F, &mut buf).unwrap();
            assert_eq!(buf, vec![b'\t', b' ']);
        }

        #[test]
        fn test_minimal_gap_with_tab() {
            let mut buf = vec![];
            format_tabs_and_spaces(7, 8, &CONFIG_E_F, &mut buf).unwrap();
            assert_eq!(buf, vec![b'\t']);
        }

        #[test]
        fn test_expanded_false_with_tab_at_end() {
            let mut buf = vec![];
            format_tabs_and_spaces(6, 8, &CONFIG_E_F, &mut buf).unwrap();
            assert_eq!(buf, vec![b'\t']);
        }
    }

    mod process_half_line {
        use super::*;

        fn create_test_config(expanded: bool, tab_size: usize) -> Config {
            Config {
                sdiff_half_width: 30,
                sdiff_column_two_offset: 60,
                tab_size,
                expanded,
                separator_pos: 15,
            }
        }

        #[test]
        fn test_empty_line_left_expanded_false() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            process_half_line(b"", 10, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf.len(), 5);
            assert_eq!(buf, vec![b'\t', b'\t', b' ', b' ', b' ']);
        }

        #[test]
        fn test_tabs_unexpanded() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            process_half_line(b"\tabc", 8, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, vec![b'\t', b'a', b'b', b'c', b'\t', b' ']);
        }

        #[test]
        fn test_utf8_multibyte() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = "😉😉😉".as_bytes();
            process_half_line(s, 3, false, false, &config, &mut buf).unwrap();
            let mut r = vec![];
            r.write_all("😉\t".as_bytes()).unwrap();
            assert_eq!(buf, r)
        }

        #[test]
        fn test_newline_handling() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            process_half_line(b"abc\ndef", 5, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, vec![b'a', b'b', b'c', b'\t', b' ', b' ']);
        }

        #[test]
        fn test_carriage_return() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            process_half_line(b"\rxyz", 5, true, false, &config, &mut buf).unwrap();
            let mut r = vec![b'\r'];
            r.extend(vec![b'\t'; 15]);
            r.extend(vec![b'x', b'y', b'z']);
            assert_eq!(buf, r);
        }

        #[test]
        fn test_exact_width_fit() {
            let config = create_test_config(true, DEF_TAB_SIZE);
            let mut buf = vec![];
            process_half_line(b"abcd", 4, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf.len(), 5);
            assert_eq!(buf, b"abcd ".to_vec());
        }

        #[test]
        fn test_non_utf8_bytes() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            // ISO-8859-1
            process_half_line(
                &[0x63, 0x61, 0x66, 0xE9],
                5,
                false,
                false,
                &config,
                &mut buf,
            )
            .unwrap();
            assert_eq!(&buf, &[0x63, 0x61, 0x66, 0xE9, b' ', b' ']);
            assert!(String::from_utf8(buf).is_err());
        }

        #[test]
        fn test_non_utf8_bytes_ignore_padding_bytes() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];

            let utf32le_bytes = [
                0x63, 0x00, 0x00, 0x00, // 'c'
                0x61, 0x00, 0x00, 0x00, // 'a'
                0x66, 0x00, 0x00, 0x00, // 'f'
                0xE9, 0x00, 0x00, 0x00, // 'é'
            ];
            // utf8 little endiand 32 bits (or 4 bytes per char)
            process_half_line(&utf32le_bytes, 6, false, false, &config, &mut buf).unwrap();
            let mut r = utf32le_bytes.to_vec();
            r.extend(vec![b' '; 3]);
            assert_eq!(buf, r);
        }

        #[test]
        fn test_non_utf8_non_preserve_ascii_bytes_cut() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];

            let gb18030 = b"\x63\x61\x66\xA8\x80"; // some random chinese encoding
                                                   //                                   ^ é char, start multi byte
            process_half_line(gb18030, 4, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, b"\x63\x61\x66\xA8 "); // break the encoding of 'é' letter
        }

        #[test]
        fn test_right_line_padding() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            process_half_line(b"xyz", 5, true, true, &config, &mut buf).unwrap();
            assert_eq!(buf.len(), 3);
        }

        #[test]
        fn test_mixed_tabs_spaces() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            process_half_line(b"\t  \t", 10, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, vec![b'\t', b' ', b' ', b'\t', b' ', b' ', b' ']);
        }

        #[test]
        fn test_overflow_multibyte() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = "日本語".as_bytes();
            process_half_line(s, 5, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, "日本  ".as_bytes());
        }

        #[test]
        fn test_white_space_gutter() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"abc";
            process_half_line(s, 3, false, true, &config, &mut buf).unwrap();
            assert_eq!(buf, b"abc\t  ");
        }

        #[test]
        fn test_expanded_true() {
            let config = create_test_config(true, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"abc";
            process_half_line(s, 10, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, b"abc        ")
        }

        #[test]
        fn test_expanded_true_with_gutter() {
            let config = create_test_config(true, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"abc";
            process_half_line(s, 10, false, true, &config, &mut buf).unwrap();
            assert_eq!(buf, b"abc          ")
        }

        #[test]
        fn test_width0_chars() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"abc\0\x0B\x07\x0C";
            process_half_line(s, 4, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, b"abc\0\x0B\x07\x0C\t ")
        }

        #[test]
        fn test_left_empty_white_space_gutter() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"";
            process_half_line(s, 9, false, true, &config, &mut buf).unwrap();
            assert_eq!(buf, b"\t\t\t");
        }

        #[test]
        fn test_s_size_eq_max_width_p1() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"abcdefghij";
            process_half_line(s, 10, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, b"abcdefghij ");
        }

        #[test]
        fn test_mixed_tabs_and_spaces_inversion() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b" \t \t ";
            process_half_line(s, 10, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, b" \t \t   ");
        }

        #[test]
        fn test_expanded_with_tabs() {
            let config = create_test_config(true, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b" \t \t ";
            process_half_line(s, 10, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, b"           ");
        }

        #[test]
        fn test_expanded_with_tabs_and_space_gutter() {
            let config = create_test_config(true, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b" \t \t ";
            process_half_line(s, 10, false, true, &config, &mut buf).unwrap();
            assert_eq!(buf, b"             ");
        }

        #[test]
        fn test_zero_width_unicode_chars() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = "\u{200B}".as_bytes();
            process_half_line(s, 10, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, "\u{200B}\t\t   ".as_bytes());
        }

        #[test]
        fn test_multiple_carriage_returns() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"\r\r";
            process_half_line(s, 10, false, false, &config, &mut buf).unwrap();
            let mut r = vec![b'\r'];
            r.extend(vec![b'\t'; 15]);
            r.push(b'\r');
            r.extend(vec![b'\t'; 15]);
            r.extend(vec![b'\t'; 2]);
            r.extend(vec![b' '; 3]);
            assert_eq!(buf, r);
        }

        #[test]
        fn test_multiple_carriage_returns_is_right_true() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"\r\r";
            process_half_line(s, 10, true, false, &config, &mut buf).unwrap();
            let mut r = vec![b'\r'];
            r.extend(vec![b'\t'; 15]);
            r.push(b'\r');
            r.extend(vec![b'\t'; 15]);
            assert_eq!(buf, r);
        }

        #[test]
        fn test_mixed_invalid_utf8_with_valid() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"abc\xFF\xFEdef";
            process_half_line(s, 10, false, false, &config, &mut buf).unwrap();
            assert!(String::from_utf8(s.to_vec()).is_err());
            assert_eq!(buf, b"abc\xFF\xFEdef   ");
        }

        #[test]
        fn test_max_width_zero() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"foo bar";
            process_half_line(s, 0, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, vec![b' ']);
        }

        #[test]
        fn test_line_only_with_tabs() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"\t\t\t";
            process_half_line(s, 10, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, vec![b'\t', b'\t', b' ', b' ', b' '])
        }

        #[test]
        fn test_tabs_expanded() {
            let config = create_test_config(true, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"\t\t\t";
            process_half_line(s, 12, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, b" ".repeat(13));
        }

        #[test]
        fn test_mixed_tabs() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"a\tb\tc\t";
            process_half_line(s, 10, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, b"a\tb\tc  ");
        }

        #[test]
        fn test_mixed_tabs_with_gutter() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"a\tb\tc\t";
            process_half_line(s, 10, false, true, &config, &mut buf).unwrap();
            assert_eq!(buf, b"a\tb\tc\t ");
        }

        #[test]
        fn test_mixed_tabs_expanded() {
            let config = create_test_config(true, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"a\tb\tc\t";
            process_half_line(s, 10, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, b"a   b   c  ");
        }

        #[test]
        fn test_mixed_tabs_expanded_with_gutter() {
            let config = create_test_config(true, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"a\tb\tc\t";
            process_half_line(s, 10, false, true, &config, &mut buf).unwrap();
            assert_eq!(buf, b"a   b   c    ");
        }

        #[test]
        fn test_break_if_invalid_max_width() {
            let config = create_test_config(true, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"a\tb\tc\t";
            process_half_line(s, 61, false, true, &config, &mut buf).unwrap();
            assert_eq!(buf, b"");
            assert_eq!(buf.len(), 0);
        }

        #[test]
        fn test_new_line() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            let s = b"abc";
            process_half_line(s, 10, false, false, &config, &mut buf).unwrap();
            assert_eq!(buf, b"abc\t\t   ");
        }
    }

    mod push_output {
        // almost all behavior of the push_output was tested with tests on process_half_line

        use super::*;

        impl Default for Config {
            fn default() -> Self {
                Config::new(130, 8, false)
            }
        }

        fn create_test_config_def() -> Config {
            Config::default()
        }

        #[test]
        fn test_left_empty_right_not_added() {
            let config = create_test_config_def();
            let left_ln = b"";
            let right_ln = b"bar";
            let symbol = b'>';
            let mut buf = vec![];
            push_output(&left_ln[..], &right_ln[..], symbol, &mut buf, &config).unwrap();
            assert_eq!(buf, b"\t\t\t\t\t\t\t      >\tbar\n");
        }

        #[test]
        fn test_right_empty_left_not_del() {
            let config = create_test_config_def();
            let left_ln = b"bar";
            let right_ln = b"";
            let symbol = b'>';
            let mut buf = vec![];
            push_output(&left_ln[..], &right_ln[..], symbol, &mut buf, &config).unwrap();
            assert_eq!(buf, b"bar\t\t\t\t\t\t\t      >\n");
        }

        #[test]
        fn test_both_empty() {
            let config = create_test_config_def();
            let left_ln = b"";
            let right_ln = b"";
            let symbol = b' ';
            let mut buf = vec![];
            push_output(&left_ln[..], &right_ln[..], symbol, &mut buf, &config).unwrap();
            assert_eq!(buf, b"\n");
        }

        #[test]
        fn test_output_cut_with_maximization() {
            let config = create_test_config_def();
            let left_ln = b"a".repeat(62);
            let right_ln = b"a".repeat(62);
            let symbol = b' ';
            let mut buf = vec![];
            push_output(&left_ln[..], &right_ln[..], symbol, &mut buf, &config).unwrap();
            assert_eq!(buf.len(), 61 * 2 + 2);
            assert_eq!(&buf[0..61], vec![b'a'; 61]);
            assert_eq!(&buf[61..62], b"\t");
            let mut end = b"a".repeat(61);
            end.push(b'\n');
            assert_eq!(&buf[62..], end);
        }

        #[test]
        fn test_both_lines_non_empty_with_space_symbol_max_tabs() {
            let config = create_test_config_def();
            let left_ln = b"left";
            let right_ln = b"right";
            let symbol = b' ';
            let mut buf = vec![];
            push_output(left_ln, right_ln, symbol, &mut buf, &config).unwrap();
            let expected_left = "left\t\t\t\t\t\t\t\t";
            let expected_right = "right";
            assert_eq!(buf, format!("{expected_left}{expected_right}\n").as_bytes());
        }

        #[test]
        fn test_non_space_symbol_with_padding() {
            let config = create_test_config_def();
            let left_ln = b"data";
            let right_ln = b"";
            let symbol = b'<'; // impossible case, just to use different symbol
            let mut buf = vec![];
            push_output(left_ln, right_ln, symbol, &mut buf, &config).unwrap();
            assert_eq!(buf, format!("data\t\t\t\t\t\t\t      <\n").as_bytes());
        }

        #[test]
        fn test_lines_exceeding_half_width() {
            let config = create_test_config_def();
            let left_ln = vec![b'a'; 100];
            let left_ln = left_ln.as_slice();
            let right_ln = vec![b'b'; 100];
            let right_ln = right_ln.as_slice();
            let symbol = b' ';
            let mut buf = vec![];
            push_output(left_ln, right_ln, symbol, &mut buf, &config).unwrap();
            let expected_left = "a".repeat(61);
            let expected_right = "b".repeat(61);
            assert_eq!(buf.len(), 61 + 1 + 61 + 1);
            assert_eq!(&buf[0..61], expected_left.as_bytes());
            assert_eq!(buf[61], b'\t');
            assert_eq!(&buf[62..123], expected_right.as_bytes());
            assert_eq!(&buf[123..], b"\n");
        }

        #[test]
        fn test_tabs_in_lines_expanded() {
            let mut config = create_test_config_def();
            config.expanded = true;
            let left_ln = b"\tleft";
            let right_ln = b"\tright";
            let symbol = b' ';
            let mut buf = vec![];
            push_output(left_ln, right_ln, symbol, &mut buf, &config).unwrap();
            let expected_left = "        left".to_string() + &" ".repeat(61 - 12);
            let expected_right = "        right";
            assert_eq!(
                buf,
                format!("{}{}{}\n", expected_left, "   ", expected_right).as_bytes()
            );
        }

        #[test]
        fn test_unicode_characters() {
            let config = create_test_config_def();
            let left_ln = "áéíóú".as_bytes();
            let right_ln = "😀😃😄".as_bytes();
            let symbol = b' ';
            let mut buf = vec![];
            push_output(left_ln, right_ln, symbol, &mut buf, &config).unwrap();
            let expected_left = format!("áéíóú\t\t\t\t\t\t\t\t");
            let expected_right = "😀😃😄";
            assert_eq!(
                buf,
                format!("{}{}\n", expected_left, expected_right).as_bytes()
            );
        }
    }

    mod diff {
        /*
        Probably this hole section should be refactored when complete sdiff
        arrives. I would say that these tests are more to document the
        behavior of the engine than to actually test whether it is right,
        because it is right, but right up to its limitations.
        */

        use super::*;

        fn generate_params() -> Params {
            Params {
                tabsize: 8,
                expand_tabs: false,
                width: 130,
                ..Default::default()
            }
        }

        fn contains_string(vec: &Vec<u8>, s: &str) -> usize {
            let pattern = s.as_bytes();
            vec.windows(pattern.len()).filter(|s| s == &pattern).count()
        }

        fn calc_lines(input: &Vec<u8>) -> usize {
            let mut lines_counter = 0;

            for c in input {
                if c == &b'\n' {
                    lines_counter += 1;
                }
            }

            lines_counter
        }

        #[test]
        fn test_equal_lines() {
            let params = generate_params();
            let from_file = b"equal";
            let to_file = b"equal";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);
            assert_eq!(calc_lines(&output), 1);
            assert!(!output.contains(&b'<'));
            assert!(!output.contains(&b'>'));
            assert_eq!(contains_string(&output, "equal"), 2)
        }

        #[test]
        fn test_different_lines() {
            let params = generate_params();
            let from_file = b"eq";
            let to_file = b"ne";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);
            assert_eq!(calc_lines(&output), 2);
            assert!(output.contains(&b'>'));
            assert!(output.contains(&b'<'));
            assert_eq!(contains_string(&output, "eq"), 1);
            assert_eq!(contains_string(&output, "ne"), 1);
        }

        #[test]
        fn test_added_line() {
            let params = generate_params();
            let from_file = b"";
            let to_file = b"new line";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert_eq!(calc_lines(&output), 1);
            assert_eq!(contains_string(&output, ">"), 1);
            assert_eq!(contains_string(&output, "new line"), 1);
        }

        #[test]
        fn test_removed_line() {
            let params = generate_params();
            let from_file = b"old line";
            let to_file = b"";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert_eq!(calc_lines(&output), 1);
            assert_eq!(contains_string(&output, "<"), 1);
            assert_eq!(contains_string(&output, "old line"), 1);
        }

        #[test]
        fn test_multiple_changes() {
            let params = generate_params();
            let from_file = b"line1\nline2\nline3";
            let to_file = b"line1\nmodified\nline4";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert_eq!(calc_lines(&output), 5);
            assert_eq!(contains_string(&output, "<"), 2);
            assert_eq!(contains_string(&output, ">"), 2);
        }

        #[test]
        fn test_unicode_and_special_chars() {
            let params = generate_params();
            let from_file = "á\t€".as_bytes();
            let to_file = "€\t😊".as_bytes();
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert!(String::from_utf8_lossy(&output).contains("á"));
            assert!(String::from_utf8_lossy(&output).contains("€"));
            assert!(String::from_utf8_lossy(&output).contains("😊"));
            assert_eq!(contains_string(&output, "<"), 1);
            assert_eq!(contains_string(&output, ">"), 1);
        }

        #[test]
        fn test_mixed_whitespace() {
            let params = generate_params();
            let from_file = b"  \tspaces";
            let to_file = b"\t\t tabs";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert!(output.contains(&b'<'));
            assert!(output.contains(&b'>'));
            assert!(String::from_utf8_lossy(&output).contains("spaces"));
            assert!(String::from_utf8_lossy(&output).contains("tabs"));
        }

        #[test]
        fn test_empty_files() {
            let params = generate_params();
            let from_file = b"";
            let to_file = b"";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert_eq!(output, vec![]);
        }

        #[test]
        fn test_partially_matching_lines() {
            let params = generate_params();
            let from_file = b"match\nchange";
            let to_file = b"match\nupdated";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert_eq!(calc_lines(&output), 3);
            assert_eq!(contains_string(&output, "match"), 2);
            assert_eq!(contains_string(&output, "<"), 1);
            assert_eq!(contains_string(&output, ">"), 1);
        }

        #[test]
        fn test_interleaved_add_remove() {
            let params = generate_params();
            let from_file = b"A\nB\nC\nD";
            let to_file = b"B\nX\nD\nY";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert_eq!(calc_lines(&output), 7);
            assert_eq!(contains_string(&output, "A"), 1);
            assert_eq!(contains_string(&output, "X"), 1);
            assert_eq!(contains_string(&output, "Y"), 1);
            assert_eq!(contains_string(&output, "<"), 3);
            assert_eq!(contains_string(&output, ">"), 3);
        }

        #[test]
        fn test_swapped_lines() {
            let params = generate_params();
            let from_file = b"1\n2\n3\n4";
            let to_file = b"4\n3\n2\n1";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert_eq!(calc_lines(&output), 7);
            assert_eq!(contains_string(&output, "<"), 3);
            assert_eq!(contains_string(&output, ">"), 3);
        }

        #[test]
        fn test_gap_between_changes() {
            let params = generate_params();
            let from_file = b"Start\nKeep1\nRemove\nKeep2\nEnd";
            let to_file = b"Start\nNew1\nKeep1\nKeep2\nNew2\nEnd";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert_eq!(calc_lines(&output), 7);
            assert_eq!(contains_string(&output, "Remove"), 1);
            assert_eq!(contains_string(&output, "New1"), 1);
            assert_eq!(contains_string(&output, "New2"), 1);
            assert_eq!(contains_string(&output, "<"), 1);
            assert_eq!(contains_string(&output, ">"), 2);
        }

        #[test]
        fn test_mixed_operations_complex() {
            let params = generate_params();
            let from_file = b"Same\nOld1\nSameMid\nOld2\nSameEnd";
            let to_file = b"Same\nNew1\nSameMid\nNew2\nNew3\nSameEnd";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert_eq!(calc_lines(&output), 8);
            assert_eq!(contains_string(&output, "<"), 2);
            assert_eq!(contains_string(&output, ">"), 3);
        }

        #[test]
        fn test_insert_remove_middle() {
            let params = generate_params();
            let from_file = b"Header\nContent1\nFooter";
            let to_file = b"Header\nContent2\nFooter";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert_eq!(calc_lines(&output), 4);
            assert_eq!(contains_string(&output, "Content1"), 1);
            assert_eq!(contains_string(&output, "Content2"), 1);
            assert_eq!(contains_string(&output, "<"), 1);
            assert_eq!(contains_string(&output, ">"), 1);
        }

        #[test]
        fn test_multiple_adjacent_changes() {
            let params = generate_params();
            let from_file = b"A\nB\nC\nD\nE";
            let to_file = b"A\nX\nY\nD\nZ";
            let mut output = vec![];
            diff(from_file, to_file, &mut output, &params);

            assert_eq!(calc_lines(&output), 8);
            assert_eq!(contains_string(&output, "<"), 3);
            assert_eq!(contains_string(&output, ">"), 3);
        }
    }

    mod config {
        use super::*;

        fn create_config(full_width: usize, tab_size: usize, expanded: bool) -> Config {
            Config::new(full_width, tab_size, expanded)
        }

        #[test]
        fn test_full_width_80_tab_4() {
            let config = create_config(80, 4, false);
            assert_eq!(config.sdiff_half_width, 37);
            assert_eq!(config.sdiff_column_two_offset, 40);
            assert_eq!(config.separator_pos, 38);
        }

        #[test]
        fn test_full_width_40_tab_8() {
            let config = create_config(40, 8, true);
            assert_eq!(config.sdiff_half_width, 16);
            assert_eq!(config.sdiff_column_two_offset, 24);
            assert_eq!(config.separator_pos, 19); // (16 +24 -1) /2 = 19.5
        }

        #[test]
        fn test_full_width_30_tab_2() {
            let config = create_config(30, 2, false);
            assert_eq!(config.sdiff_half_width, 13);
            assert_eq!(config.sdiff_column_two_offset, 16);
            assert_eq!(config.separator_pos, 14);
        }

        #[test]
        fn test_small_width_10_tab_4() {
            let config = create_config(10, 4, false);
            assert_eq!(config.sdiff_half_width, 2);
            assert_eq!(config.sdiff_column_two_offset, 8);
            assert_eq!(config.separator_pos, 4);
        }

        #[test]
        fn test_minimal_width_3_tab_4() {
            let config = create_config(3, 4, false);
            assert_eq!(config.sdiff_half_width, 0);
            assert_eq!(config.sdiff_column_two_offset, 3);
            assert_eq!(config.separator_pos, 1);
        }

        #[test]
        fn test_odd_width_7_tab_3() {
            let config = create_config(7, 3, false);
            assert_eq!(config.sdiff_half_width, 1);
            assert_eq!(config.sdiff_column_two_offset, 6);
            assert_eq!(config.separator_pos, 3);
        }

        #[test]
        fn test_tab_size_larger_than_width() {
            let config = create_config(5, 10, false);
            assert_eq!(config.sdiff_half_width, 0);
            assert_eq!(config.sdiff_column_two_offset, 5);
            assert_eq!(config.separator_pos, 2);
        }
    }
}
