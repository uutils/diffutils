// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use core::cmp::{max, min};
use std::{io::Write, vec};
use unicode_width::UnicodeWidthStr;

use crate::params::Params;
use crate::ses;
use crate::utils::comparison_lines_inclusive;

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

        // Pad to the middle of the gutter so the marker doesn't drift on wider gutters (#269).
        format_tabs_and_spaces(half_width + 1, separator_pos, config, output)?;
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

    let mut left_lines = comparison_lines_inclusive(from_file, params);
    let mut right_lines = comparison_lines_inclusive(to_file, params);
    let config = Config::new(params.width, params.tabsize, params.expand_tabs);

    if left_lines.last().is_some_and(|l| l.raw.is_empty()) {
        left_lines.pop();
    }

    if right_lines.last().is_some_and(|l| l.raw.is_empty()) {
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
    let differences = ses::diff_lines(&left_lines, &right_lines);

    for change in differences.iter_changes() {
        match change {
            ses::Change::Delete(left_ln) => {
                push_output(left_ln, b"", b'<', output, &config).unwrap()
            }
            ses::Change::Insert(right_ln) => {
                push_output(b"", right_ln, b'>', output, &config).unwrap()
            }
            ses::Change::Equal(left_ln, right_ln) => {
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
    use rstest::rstest;

    mod format_tabs_and_spaces {
        use super::*;

        #[rstest]
        #[case::expanded_false(0, 5, false, b"\t ")]
        #[case::expanded_true(0, 5, true, b"     ")]
        #[case::from_greater_than_to(6, 5, false, b"")]
        #[case::from_non_zero_position(2, 7, false, b"\t   ")]
        #[case::multiple_full_tabs_needed(0, 12, false, b"\t\t\t")]
        #[case::uneven_tab_boundary_with_spaces(3, 10, false, b"\t\t  ")]
        #[case::expanded_true_with_offset(3, 9, true, b"      ")]
        #[case::exact_tab_boundary_from_midpoint(4, 8, false, b"\t")]
        #[case::mixed_tabs_and_spaces_edge_case(5, 9, false, b"\t ")]
        #[case::minimal_gap_with_tab(7, 8, false, b"\t")]
        #[case::expanded_false_with_tab_at_end(6, 8, false, b"\t")]
        fn format(
            #[case] from: usize,
            #[case] to: usize,
            #[case] expanded: bool,
            #[case] expected: &[u8],
        ) {
            let config = Config {
                sdiff_half_width: 60,
                tab_size: DEF_TAB_SIZE,
                expanded,
                sdiff_column_two_offset: 0,
                separator_pos: 0,
            };
            let mut buf = vec![];
            format_tabs_and_spaces(from, to, &config, &mut buf).unwrap();
            assert_eq!(buf, expected);
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

        #[rstest]
        #[case::empty_line_left_expanded_false(b"", 10, false, false, false, b"\t\t   ")]
        #[case::tabs_unexpanded(b"\tabc", 8, false, false, false, b"\tabc\t ")]
        #[case::utf8_multibyte("😉😉😉".as_bytes(), 3, false, false, false, "😉\t".as_bytes())]
        #[case::newline_handling(b"abc\ndef", 5, false, false, false, b"abc\t  ")]
        #[case::carriage_return(
            b"\rxyz",
            5,
            true,
            false,
            false,
            &[b"\r".as_slice(), &[b'\t'; 15], b"xyz"].concat()
        )]
        #[case::exact_width_fit(b"abcd", 4, false, false, true, b"abcd ")]
        // ISO-8859-1
        #[case::non_utf8_bytes(b"caf\xE9", 5, false, false, false, b"caf\xE9  ")]
        // UTF-32LE, 4 bytes per char: 'c', 'a', 'f', 'é'
        #[case::non_utf8_bytes_ignore_padding_bytes(
            b"c\0\0\0a\0\0\0f\0\0\0\xE9\0\0\0",
            6,
            false,
            false,
            false,
            b"c\0\0\0a\0\0\0f\0\0\0\xE9\0\0\0   "
        )]
        // GB18030: \xA8\x80 is 'é', the cut breaks its encoding
        #[case::non_utf8_non_preserve_ascii_bytes_cut(
            b"caf\xA8\x80",
            4,
            false,
            false,
            false,
            b"caf\xA8 "
        )]
        #[case::mixed_tabs_spaces(b"\t  \t", 10, false, false, false, b"\t  \t   ")]
        #[case::overflow_multibyte("日本語".as_bytes(), 5, false, false, false, "日本  ".as_bytes())]
        #[case::white_space_gutter(b"abc", 3, false, true, false, b"abc\t  ")]
        #[case::expanded_true(b"abc", 10, false, false, true, b"abc        ")]
        #[case::expanded_true_with_gutter(b"abc", 10, false, true, true, b"abc          ")]
        #[case::width0_chars(b"abc\0\x0B\x07\x0C", 4, false, false, false, b"abc\0\x0B\x07\x0C\t ")]
        #[case::left_empty_white_space_gutter(b"", 9, false, true, false, b"\t\t\t")]
        #[case::s_size_eq_max_width_p1(b"abcdefghij", 10, false, false, false, b"abcdefghij ")]
        #[case::mixed_tabs_and_spaces_inversion(b" \t \t ", 10, false, false, false, b" \t \t   ")]
        #[case::expanded_with_tabs(b" \t \t ", 10, false, false, true, b"           ")]
        #[case::expanded_with_tabs_and_space_gutter(
            b" \t \t ",
            10,
            false,
            true,
            true,
            b"             "
        )]
        #[case::zero_width_unicode_chars(
            "\u{200B}".as_bytes(),
            10,
            false,
            false,
            false,
            "\u{200B}\t\t   ".as_bytes()
        )]
        #[case::multiple_carriage_returns(
            b"\r\r",
            10,
            false,
            false,
            false,
            &[b"\r".as_slice(), &[b'\t'; 15], b"\r", &[b'\t'; 17], b"   "].concat()
        )]
        #[case::multiple_carriage_returns_is_right_true(
            b"\r\r",
            10,
            true,
            false,
            false,
            &[b"\r".as_slice(), &[b'\t'; 15], b"\r", &[b'\t'; 15]].concat()
        )]
        #[case::mixed_invalid_utf8_with_valid(
            b"abc\xFF\xFEdef",
            10,
            false,
            false,
            false,
            b"abc\xFF\xFEdef   "
        )]
        #[case::max_width_zero(b"foo bar", 0, false, false, false, b" ")]
        #[case::line_only_with_tabs(b"\t\t\t", 10, false, false, false, b"\t\t   ")]
        #[case::tabs_expanded(b"\t\t\t", 12, false, false, true, &b" ".repeat(13))]
        #[case::mixed_tabs(b"a\tb\tc\t", 10, false, false, false, b"a\tb\tc  ")]
        #[case::mixed_tabs_with_gutter(b"a\tb\tc\t", 10, false, true, false, b"a\tb\tc\t ")]
        #[case::mixed_tabs_expanded(b"a\tb\tc\t", 10, false, false, true, b"a   b   c  ")]
        #[case::mixed_tabs_expanded_with_gutter(
            b"a\tb\tc\t",
            10,
            false,
            true,
            true,
            b"a   b   c    "
        )]
        #[case::break_if_invalid_max_width(b"a\tb\tc\t", 61, false, true, true, b"")]
        #[case::new_line(b"abc", 10, false, false, false, b"abc\t\t   ")]
        fn output(
            #[case] line: &[u8],
            #[case] max_width: usize,
            #[case] is_right: bool,
            #[case] white_space_gutter: bool,
            #[case] expanded: bool,
            #[case] expected: &[u8],
        ) {
            let config = create_test_config(expanded, DEF_TAB_SIZE);
            let mut buf = vec![];
            process_half_line(
                line,
                max_width,
                is_right,
                white_space_gutter,
                &config,
                &mut buf,
            )
            .unwrap();
            assert_eq!(buf, expected);
        }

        #[test]
        fn test_right_line_padding() {
            let config = create_test_config(false, DEF_TAB_SIZE);
            let mut buf = vec![];
            process_half_line(b"xyz", 5, true, true, &config, &mut buf).unwrap();
            assert_eq!(buf.len(), 3);
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

        #[rstest]
        #[case::left_empty_right_not_added("", "bar", b'>', false, "\t\t\t\t\t\t\t      >\tbar\n")]
        #[case::right_empty_left_not_del("bar", "", b'>', false, "bar\t\t\t\t\t\t\t      >\n")]
        #[case::both_empty("", "", b' ', false, "\n")]
        #[case::output_cut_with_maximization(
            &"a".repeat(62),
            &"a".repeat(62),
            b' ',
            false,
            &format!("{0}\t{0}\n", "a".repeat(61))
        )]
        #[case::both_lines_non_empty_with_space_symbol_max_tabs(
            "left",
            "right",
            b' ',
            false,
            "left\t\t\t\t\t\t\t\tright\n"
        )]
        // impossible case, just to use different symbol
        #[case::non_space_symbol_with_padding(
            "data",
            "",
            b'<',
            false,
            "data\t\t\t\t\t\t\t      <\n"
        )]
        #[case::lines_exceeding_half_width(
            &"a".repeat(100),
            &"b".repeat(100),
            b' ',
            false,
            &format!("{}\t{}\n", "a".repeat(61), "b".repeat(61))
        )]
        #[case::tabs_in_lines_expanded(
            "\tleft",
            "\tright",
            b' ',
            true,
            &format!("{:<61}   {}\n", "        left", "        right")
        )]
        #[case::unicode_characters("áéíóú", "😀😃😄", b' ', false, "áéíóú\t\t\t\t\t\t\t\t😀😃😄\n")]
        fn output(
            #[case] left_ln: &str,
            #[case] right_ln: &str,
            #[case] symbol: u8,
            #[case] expanded: bool,
            #[case] expected: &str,
        ) {
            let config = Config {
                expanded,
                ..Config::default()
            };
            let mut buf = vec![];
            push_output(
                left_ln.as_bytes(),
                right_ln.as_bytes(),
                symbol,
                &mut buf,
                &config,
            )
            .unwrap();
            assert_eq!(buf, expected.as_bytes());
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

        fn calc_lines(input: &[u8]) -> usize {
            input.iter().filter(|&&c| c == b'\n').count()
        }

        #[rstest]
        #[case::equal_lines("equal", "equal", 1, 0, 0, &[("equal", 2)])]
        #[case::different_lines("eq", "ne", 2, 1, 1, &[("eq", 1), ("ne", 1)])]
        #[case::added_line("", "new line", 1, 0, 1, &[("new line", 1)])]
        #[case::removed_line("old line", "", 1, 1, 0, &[("old line", 1)])]
        #[case::multiple_changes("line1\nline2\nline3", "line1\nmodified\nline4", 5, 2, 2, &[])]
        #[case::unicode_and_special_chars(
            "á\t€",
            "€\t😊",
            2,
            1,
            1,
            &[("á", 1), ("€", 2), ("😊", 1)]
        )]
        #[case::mixed_whitespace("  \tspaces", "\t\t tabs", 2, 1, 1, &[("spaces", 1), ("tabs", 1)])]
        #[case::partially_matching_lines("match\nchange", "match\nupdated", 3, 1, 1, &[("match", 2)])]
        #[case::interleaved_add_remove(
            "A\nB\nC\nD",
            "B\nX\nD\nY",
            7,
            3,
            3,
            &[("A", 1), ("X", 1), ("Y", 1)]
        )]
        #[case::swapped_lines("1\n2\n3\n4", "4\n3\n2\n1", 7, 3, 3, &[])]
        #[case::gap_between_changes(
            "Start\nKeep1\nRemove\nKeep2\nEnd",
            "Start\nNew1\nKeep1\nKeep2\nNew2\nEnd",
            7,
            1,
            2,
            &[("Remove", 1), ("New1", 1), ("New2", 1)]
        )]
        #[case::mixed_operations_complex(
            "Same\nOld1\nSameMid\nOld2\nSameEnd",
            "Same\nNew1\nSameMid\nNew2\nNew3\nSameEnd",
            8,
            2,
            3,
            &[]
        )]
        #[case::insert_remove_middle(
            "Header\nContent1\nFooter",
            "Header\nContent2\nFooter",
            4,
            1,
            1,
            &[("Content1", 1), ("Content2", 1)]
        )]
        #[case::multiple_adjacent_changes("A\nB\nC\nD\nE", "A\nX\nY\nD\nZ", 8, 3, 3, &[])]
        fn output(
            #[case] from_file: &str,
            #[case] to_file: &str,
            #[case] lines: usize,
            #[case] removed: usize,
            #[case] added: usize,
            #[case] words: &[(&str, usize)],
        ) {
            let params = generate_params();
            let mut output = vec![];
            diff(
                from_file.as_bytes(),
                to_file.as_bytes(),
                &mut output,
                &params,
            );

            assert_eq!(calc_lines(&output), lines);
            assert_eq!(contains_string(&output, "<"), removed);
            assert_eq!(contains_string(&output, ">"), added);
            for (word, count) in words {
                assert_eq!(contains_string(&output, word), *count, "{word:?}");
            }
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
        fn test_empty_files() {
            let params = generate_params();
            let mut output = vec![];
            diff(b"", b"", &mut output, &params);
            assert_eq!(output, vec![]);
        }
    }

    mod config {
        use super::*;

        #[rstest]
        #[case::full_width_80_tab_4(80, 4, false, 37, 40, 38)]
        // (16 + 24 - 1) / 2 = 19.5
        #[case::full_width_40_tab_8(40, 8, true, 16, 24, 19)]
        #[case::full_width_30_tab_2(30, 2, false, 13, 16, 14)]
        #[case::small_width_10_tab_4(10, 4, false, 2, 8, 4)]
        #[case::minimal_width_3_tab_4(3, 4, false, 0, 3, 1)]
        #[case::odd_width_7_tab_3(7, 3, false, 1, 6, 3)]
        // `tab_size + GUTTER_WIDTH_MIN` must not overflow `isize`.
        #[case::max_tab_size(130, crate::params::MAX_TABSIZE, false, 0, 130, 64)]
        #[case::tab_size_larger_than_width(5, 10, false, 0, 5, 2)]
        fn new(
            #[case] full_width: usize,
            #[case] tab_size: usize,
            #[case] expanded: bool,
            #[case] half_width: usize,
            #[case] column_two_offset: usize,
            #[case] separator_pos: usize,
        ) {
            let config = Config::new(full_width, tab_size, expanded);
            assert_eq!(config.sdiff_half_width, half_width);
            assert_eq!(config.sdiff_column_two_offset, column_two_offset);
            assert_eq!(config.separator_pos, separator_pos);
        }
    }
}
