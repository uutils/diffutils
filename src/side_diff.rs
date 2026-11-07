// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use core::cmp::min;
use diff::Result;
use std::{io::Write, vec};
use unicode_width::UnicodeWidthChar;

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
    // (bytes for the next char, visible width). The width is None when the
    // bytes are not a character.
    type Item = (&'a [u8], Option<usize>);

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
                // A control character has no width, it does not move the column.
                let width = c.chars().next().and_then(UnicodeWidthChar::width);
                Some((view, Some(width.unwrap_or(0))))
            }
            Err(_) => {
                // We did not find an utf-8 char within the next 4 bytes, return the single byte.
                self.current = &self.current[1..];
                Some((&view[..1], None))
            }
        }
    }
}

impl Config {
    pub fn new(full_width: usize, tab_size: usize, expanded: bool) -> Self {
        let tab_size = tab_size.max(1);

        let (half_width, column_two_offset) =
            Self::layout(full_width, if expanded { 1 } else { tab_size });

        Self {
            expanded,
            sdiff_column_two_offset: column_two_offset,
            tab_size,
            sdiff_half_width: half_width,
            separator_pos: (half_width + column_two_offset).saturating_sub(1) >> 1,
        }
    }

    fn layout(full_width: usize, tab_size: usize) -> (usize, usize) {
        debug_assert!(tab_size != 0);

        // The offset is a multiple of tab_size, so a tab stop wider than the
        // whole line leaves only two possibilities, zero, or past the right
        // edge. Neither leaves room for a second column, so the layout
        // collapses to one
        if tab_size > full_width {
            return (0, full_width);
        }

        // Column two starts at the tab stop nearest the midpoint of the two
        // halves. midpoint instead of (full_width + span) / 2 because that
        // sum does not fit for a width near usize::MAX. The saturation covers
        // a value within GUTTER_WIDTH_MIN of usize::MAX on a line of equal width.
        let span = tab_size.saturating_add(GUTTER_WIDTH_MIN);
        let balance = full_width.midpoint(span);
        let offset = balance - balance % tab_size;

        // If either bound would go negative, the half line does not fit.
        let half_width = min(
            offset.saturating_sub(GUTTER_WIDTH_MIN),
            full_width.saturating_sub(offset),
        );

        // If it does not fit, one column spans the whole line.
        if half_width == 0 {
            (0, full_width)
        } else {
            (half_width, offset)
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

    loop {
        let advance = tab_size - current % tab_size;

        if advance > to - current {
            break;
        }

        buf.write_all(b"\t")?;
        current += advance;
    }

    while current < to {
        buf.write_all(b" ")?;
        current += 1;
    }

    Ok(())
}

// Print the text of one half of a row, cut at out_bound columns, and return
// the column where the output stopped. indent is the column the half starts
// at, a carriage return goes back to it.
fn print_half_line<T: Write>(
    line: &[u8],
    indent: usize,
    out_bound: usize,
    config: &Config,
    buf: &mut T,
) -> std::io::Result<usize> {
    let expanded = config.expanded;
    let tab_size = config.tab_size;

    // in_position is the column the text has reached, out_position the one
    // the output has. They part once something is cut.
    let mut in_position: usize = 0;
    let mut out_position: usize = 0;

    for (char, width) in CharIter::from(line) {
        match char {
            b"\t" => {
                let spaces = tab_size - in_position % tab_size;

                if in_position == out_position {
                    let tabstop = out_position.saturating_add(spaces);

                    if expanded {
                        while out_position < min(tabstop, out_bound) {
                            buf.write_all(b" ")?;
                            out_position += 1;
                        }
                    } else if tabstop < out_bound {
                        out_position = tabstop;
                        buf.write_all(b"\t")?;
                    }
                }

                in_position = in_position.saturating_add(spaces);
            }
            b"\n" => {
                break;
            }
            b"\r" => {
                buf.write_all(b"\r")?;
                format_tabs_and_spaces(0, indent, config, buf)?;
                in_position = 0;
                out_position = 0;
            }
            b"\x08" => {
                if in_position != 0 {
                    in_position -= 1;

                    if in_position < out_bound {
                        if out_position <= in_position {
                            // make up for a tab that was cut
                            while out_position < in_position {
                                buf.write_all(b" ")?;
                                out_position += 1;
                            }
                        } else {
                            out_position = in_position;
                            buf.write_all(char)?;
                        }
                    }
                }
            }
            b"\0" | b"\x0C" | b"\x0B" => {
                if in_position < out_bound {
                    buf.write_all(char)?;
                }
            }
            _ => match width {
                Some(width) => {
                    in_position = in_position.saturating_add(width);

                    if in_position <= out_bound {
                        out_position = in_position;
                        buf.write_all(char)?;
                    }
                }
                None => {
                    if in_position < out_bound {
                        buf.write_all(char)?;
                    }
                }
            },
        }
    }

    Ok(out_position)
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

    // Every padding starts at the column the output is at, so a short or a
    // blank left line never leaves the right one out of place.
    let mut column = 0;

    if !left_ln.is_empty() {
        column = print_half_line(left_ln, 0, half_width, config, output)?;
    }

    if symbol != b' ' {
        // The marker sits at the middle of the gutter.
        format_tabs_and_spaces(column, separator_pos, config, output)?;
        output.write_all(&[symbol])?;
        column = separator_pos + 1;
    }

    // A blank right line is not padded, the row ends where it is.
    if !right_ln.is_empty() && right_ln[0] != b'\n' {
        format_tabs_and_spaces(column, column_two_offset, config, output)?;
        print_half_line(right_ln, column_two_offset, half_width, config, output)?;
    }

    if put_new_line {
        writeln!(output)?;
    }

    Ok(())
}

pub fn diff<T: Write>(
    from_file: &[u8], // The left file
    to_file: &[u8],   // The right file
    output: &mut T,
    params: &Params,
) -> Vec<u8> {
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

    mod layout {
        use super::*;

        #[track_caller]
        fn assert_layout(
            full_width: usize,
            tab_size: usize,
            half_width: usize,
            column_two_offset: usize,
            separator_pos: usize,
        ) {
            assert_config(
                full_width,
                tab_size,
                false,
                half_width,
                column_two_offset,
                separator_pos,
            );
        }

        #[track_caller]
        fn assert_layout_expanded(
            full_width: usize,
            tab_size: usize,
            half_width: usize,
            column_two_offset: usize,
            separator_pos: usize,
        ) {
            assert_config(
                full_width,
                tab_size,
                true,
                half_width,
                column_two_offset,
                separator_pos,
            );
        }

        #[track_caller]
        fn assert_config(
            full_width: usize,
            tab_size: usize,
            expanded: bool,
            half_width: usize,
            column_two_offset: usize,
            separator_pos: usize,
        ) {
            let config = Config::new(full_width, tab_size, expanded);

            assert_eq!(config.sdiff_half_width, half_width, "half width");
            assert_eq!(
                config.sdiff_column_two_offset, column_two_offset,
                "column two offset"
            );
            assert_eq!(config.separator_pos, separator_pos, "separator pos");
        }

        #[test]
        fn default_width_and_tab_size() {
            assert_layout(130, 8, 61, 64, 62);
        }

        #[test]
        fn expanded_tabs_lay_out_as_a_stop_on_every_column() {
            assert_layout(130, 1, 63, 67, 64);
            assert_layout_expanded(130, 8, 63, 67, 64);
            assert_layout_expanded(130, usize::MAX, 63, 67, 64);
        }

        #[test]
        fn expanded_tabs_widen_the_half_line() {
            assert_layout(40, 8, 16, 24, 19);
            assert_layout_expanded(40, 8, 18, 22, 19);
        }

        #[test]
        fn expanded_tabs_keep_the_real_tab_size_for_rendering() {
            assert_eq!(Config::new(130, 8, true).tab_size, 8);
            assert_eq!(Config::new(130, 8, false).tab_size, 8);
        }

        #[test]
        fn expanded_tabs_reach_the_next_real_tab_stop() {
            let params = Params {
                width: 40,
                tabsize: 8,
                expand_tabs: true,
                ..Default::default()
            };
            let mut output = vec![];

            diff(b"a\tb\n", b"a\tc\n", &mut output, &params);

            assert!(!output.contains(&b'\t'), "expanded output still has tabs");
            assert!(
                output.starts_with(b"a       b"),
                "tab did not reach column 8"
            );
        }

        #[test]
        fn unexpanded_tabs_stay_tabs() {
            let params = Params {
                width: 40,
                tabsize: 8,
                ..Default::default()
            };
            let mut output = vec![];

            diff(b"a\tb\n", b"a\tc\n", &mut output, &params);

            assert!(output.starts_with(b"a\tb"));
        }

        #[test]
        fn common_widths() {
            assert_layout(80, 8, 37, 40, 38);
            assert_layout(10, 7, 3, 7, 4);
        }

        #[test]
        fn tab_size_wider_than_the_gutter_still_leaves_two_columns() {
            assert_layout(10, 8, 2, 8, 4);
        }

        #[test]
        fn tab_size_equal_to_the_width_leaves_one_column() {
            assert_layout(10, 10, 0, 10, 4);
        }

        #[test]
        fn tab_size_wider_than_the_width_leaves_one_column() {
            assert_layout(10, 11, 0, 10, 4);
        }

        #[test]
        fn smallest_width_the_cli_accepts() {
            assert_layout(1, 8, 0, 1, 0);
        }

        #[test]
        fn column_two_offset_past_the_right_edge() {
            assert_layout(5, 8, 0, 5, 2);
        }

        #[test]
        fn huge_tab_size() {
            assert_layout(130, usize::MAX, 0, 130, 64);
            assert_layout(130, usize::MAX / 2, 0, 130, 64);
            assert_layout(130, 9223372036854775805, 0, 130, 64);
        }

        #[test]
        fn huge_width() {
            let half = usize::MAX / 2 - 2;
            let offset = usize::MAX / 2 + 1;
            assert_layout(usize::MAX, 8, half, offset, (half + offset - 1) >> 1);
        }

        #[test]
        fn huge_width_and_tab_size() {
            assert_layout(usize::MAX, usize::MAX, 0, usize::MAX, usize::MAX >> 1);
        }

        #[test]
        fn zero_width() {
            assert_layout(0, 8, 0, 0, 0);
        }

        #[test]
        fn zero_tab_size_behaves_like_one() {
            assert_layout(130, 0, 63, 67, 64);
            assert_layout(130, 1, 63, 67, 64);
        }

        #[track_caller]
        fn assert_renders(from: &[u8], to: &[u8], width: usize, tabsize: usize) {
            for expand_tabs in [false, true] {
                let params = Params {
                    width,
                    tabsize,
                    expand_tabs,
                    ..Default::default()
                };
                let mut output = vec![];

                diff(from, to, &mut output, &params);

                if expand_tabs {
                    assert!(
                        !output.contains(&b'\t'),
                        "tab in expanded output, width {width}, tab size {tabsize}"
                    );
                }
            }
        }

        #[track_caller]
        fn assert_output(from: &[u8], to: &[u8], width: usize, expand_tabs: bool, expected: &str) {
            let params = Params {
                width,
                tabsize: 8,
                expand_tabs,
                ..Default::default()
            };
            let mut output = vec![];

            diff(from, to, &mut output, &params);

            assert_eq!(String::from_utf8_lossy(&output), expected);
        }

        #[test]
        fn changed_line_output() {
            assert_output(
                b"a\tb\n",
                b"a\tc\n",
                130,
                false,
                "a\tb\t\t\t\t\t\t      <\n\t\t\t\t\t\t\t      >\ta\tc\n",
            );
            assert_output(
                b"a\tb\n",
                b"a\tc\n",
                40,
                true,
                "a       b          <\n                   >  a       c\n",
            );
        }

        #[test]
        fn common_line_starts_at_column_two() {
            assert_output(
                b"a\nb\nc\n",
                b"a\nB\nc\n",
                40,
                false,
                "a\t\t\ta\nb\t\t   <\n\t\t   >\tB\nc\t\t\tc\n",
            );
            assert_output(
                b"a\nb\nc\n",
                b"a\nB\nc\n",
                40,
                true,
                "a                     a\nb                  <\n                   >  B\nc                     c\n",
            );
        }

        #[test]
        fn common_blank_line_is_left_empty() {
            assert_output(
                b"a\n\nc\n",
                b"a\n\nc\n",
                130,
                false,
                "a\t\t\t\t\t\t\t\ta\n\nc\t\t\t\t\t\t\t\tc\n",
            );
            assert_output(
                b"a\n\nc\n",
                b"a\n\nc\n",
                40,
                true,
                "a                     a\n\nc                     c\n",
            );
        }

        #[test]
        fn tab_reaching_the_end_of_the_right_half_is_cut() {
            assert_output(
                b"  \tspaces\n",
                b"\t\t tabs\n",
                40,
                false,
                "  \tspaces\t   <\n\t\t   >\t\t\n",
            );
        }

        #[test]
        fn unchanged_line_output() {
            assert_output(
                b"same\n",
                b"same\n",
                130,
                false,
                "same\t\t\t\t\t\t\t\tsame\n",
            );
        }

        #[test]
        fn extreme_tab_size_renders() {
            for tabsize in [
                0,
                1,
                2,
                7,
                8,
                1000,
                usize::MAX / 2,
                usize::MAX - 1,
                usize::MAX,
            ] {
                for width in [0, 1, 2, 3, 5, 10, 40, 130, 1000, 65535] {
                    assert_renders(b"a\tb\n", b"a\tc\n", width, tabsize);
                }
            }
        }

        #[test]
        fn padding_to_the_end_of_the_widest_line_and_tab_size() {
            let config = Config::new(usize::MAX, usize::MAX, false);
            let mut output = vec![];

            format_tabs_and_spaces(0, usize::MAX, &config, &mut output).unwrap();

            assert_eq!(output, b"\t");
        }

        #[test]
        fn every_small_width_and_tab_size_renders() {
            let cases: [(&[u8], &[u8]); 3] = [
                (b"aaa\tbbb\n", b"aaa\tccc\n"),
                (b"\t\t\n", b"\n"),
                ("\u{4f60}\u{597d}\t\u{1f600}\n".as_bytes(), b"a\n"),
            ];
            for (from, to) in cases {
                for width in 0..96 {
                    for tabsize in 0..96 {
                        assert_renders(from, to, width, tabsize);
                    }
                }
            }
        }
    }

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

    mod print_half_line {
        use super::*;

        #[track_caller]
        fn assert_half_line(
            line: &[u8],
            indent: usize,
            out_bound: usize,
            expanded: bool,
            expected: &[u8],
            expected_column: usize,
        ) {
            let config = Config {
                sdiff_half_width: out_bound,
                sdiff_column_two_offset: indent,
                tab_size: 8,
                expanded,
                separator_pos: 0,
            };
            let mut buf = vec![];
            let column = print_half_line(line, indent, out_bound, &config, &mut buf).unwrap();

            assert_eq!(buf, expected);
            assert_eq!(column, expected_column);
        }

        #[test]
        fn test_text_that_fits() {
            assert_half_line(b"abc", 0, 10, false, b"abc", 3);
        }

        #[test]
        fn test_empty_line() {
            assert_half_line(b"", 0, 10, false, b"", 0);
        }

        #[test]
        fn test_stops_at_the_newline() {
            assert_half_line(b"abc\ndef", 0, 10, false, b"abc", 3);
        }

        #[test]
        fn test_cut_at_the_bound() {
            assert_half_line(b"abcdef", 0, 4, false, b"abcd", 4);
        }

        #[test]
        fn test_zero_bound() {
            assert_half_line(b"abc", 0, 0, false, b"", 0);
        }

        #[test]
        fn test_tab_inside_the_bound() {
            assert_half_line(b"a\tb", 0, 16, false, b"a\tb", 9);
        }

        #[test]
        fn test_tab_reaching_the_bound_is_cut() {
            // the text after the tab is past the bound, even without the tab
            assert_half_line(b"a\tb", 0, 8, false, b"a", 1);
            assert_half_line(b"\tab", 0, 8, false, b"", 0);
        }

        #[test]
        fn test_only_the_first_cut_tab_counts() {
            assert_half_line(b"a\t\t\tb", 0, 12, false, b"a\t", 8);
        }

        #[test]
        fn test_expanded_tab() {
            assert_half_line(b"a\tb", 0, 16, true, b"a       b", 9);
        }

        #[test]
        fn test_expanded_tab_stops_at_the_bound() {
            assert_half_line(b"a\tb", 0, 5, true, b"a    ", 5);
        }

        #[test]
        fn test_wide_characters() {
            assert_half_line("日本語".as_bytes(), 0, 5, false, "日本".as_bytes(), 4);
        }

        #[test]
        fn test_zero_width_character_at_the_bound() {
            let line = "abc\u{301}d".as_bytes();
            assert_half_line(line, 0, 3, false, "abc\u{301}".as_bytes(), 3);
        }

        #[test]
        fn test_bytes_that_are_not_a_character_take_no_column() {
            assert_half_line(b"caf\xE9!", 0, 4, false, b"caf\xE9!", 4);
            assert_half_line(b"cafe\xE9", 0, 4, false, b"cafe", 4);
        }

        #[test]
        fn test_control_characters_take_no_column() {
            assert_half_line(b"a\0\x0C\x0B\x07b", 0, 2, false, b"a\0\x0C\x0B\x07b", 2);
        }

        #[test]
        fn test_carriage_return_goes_back_to_the_indent() {
            assert_half_line(b"ab\rcd", 0, 10, false, b"ab\rcd", 2);
            assert_half_line(b"ab\rcd", 16, 10, false, b"ab\r\t\tcd", 2);
            assert_half_line(b"ab\rcd", 3, 10, true, b"ab\r   cd", 2);
        }

        #[test]
        fn test_carriage_return_starts_the_bound_over() {
            assert_half_line(b"abcdef\rghi", 0, 4, false, b"abcd\rghi", 3);
        }

        #[test]
        fn test_backspace() {
            assert_half_line(b"ab\x08c", 0, 10, false, b"ab\x08c", 2);
            assert_half_line(b"\x08a", 0, 10, false, b"a", 1);
        }

        #[test]
        fn test_backspace_after_a_cut_tab_pads_with_spaces() {
            assert_half_line(b"a\t\x08b", 0, 8, false, b"a      b", 8);
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
            assert_eq!(buf, "data\t\t\t\t\t\t\t      <\n".as_bytes());
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
            let expected_left = "áéíóú\t\t\t\t\t\t\t\t";
            let expected_right = "😀😃😄";
            assert_eq!(buf, format!("{expected_left}{expected_right}\n").as_bytes());
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

        fn contains_string(vec: &[u8], s: &str) -> usize {
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

        // Regression for #269: the `-y` marker must sit at the gutter middle at any width.
        #[test]
        fn test_gutter_marker_column_wide_gutter() {
            let params = Params {
                tabsize: 8,
                expand_tabs: true,
                width: 40,
                ..Default::default()
            };
            let mut output = vec![];
            diff(b"aa\n", b"", &mut output, &params);
            let text = String::from_utf8(output).unwrap();
            let line = text.lines().next().unwrap();
            assert_eq!(
                line.find('<'),
                Some(19),
                "gutter marker '<' should be at column 19 for --width=40, got: {line:?}"
            );
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
            assert_eq!(config.sdiff_half_width, 18);
            assert_eq!(config.sdiff_column_two_offset, 22);
            assert_eq!(config.separator_pos, 19); // (18 + 22 - 1) / 2 = 19.5
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
