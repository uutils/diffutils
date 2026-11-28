// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::io::Write;

use crate::params::Params;
use crate::utils::{comparison_lines, do_write_line};

#[derive(Debug, PartialEq)]
struct Mismatch {
    pub line_number_expected: usize,
    pub line_number_actual: usize,
    pub expected: Vec<Vec<u8>>,
    pub actual: Vec<Vec<u8>>,
}

impl Mismatch {
    fn new(line_number_expected: usize, line_number_actual: usize) -> Mismatch {
        Mismatch {
            line_number_expected,
            line_number_actual,
            expected: Vec::new(),
            actual: Vec::new(),
        }
    }
}

// Produces a diff between the expected output and actual output.
fn make_diff(expected: &[u8], actual: &[u8], params: &Params) -> Vec<Mismatch> {
    let stop_early = params.brief;
    let mut line_number_expected = 1;
    let mut line_number_actual = 1;
    let mut results = Vec::new();
    let mut mismatch = Mismatch::new(line_number_expected, line_number_actual);

    let mut expected_lines = comparison_lines(expected, params);
    let mut actual_lines = comparison_lines(actual, params);

    debug_assert_eq!(b"".split(|&c| c == b'\n').count(), 1);
    // ^ means that underflow here is impossible
    let _expected_lines_count = expected_lines.len() - 1;
    let _actual_lines_count = actual_lines.len() - 1;

    // Like GNU diff, a last line without its newline compares equal to the
    // same line with one: an ed script cannot say that a newline is missing,
    // so the caller warns about the file instead.
    if expected_lines.last().is_some_and(|l| l.raw.is_empty()) {
        expected_lines.pop();
    }
    if actual_lines.last().is_some_and(|l| l.raw.is_empty()) {
        actual_lines.pop();
    }

    for result in diff::slice(&expected_lines, &actual_lines) {
        match result {
            diff::Result::Left(str) => {
                if !mismatch.actual.is_empty() {
                    results.push(mismatch);
                    mismatch = Mismatch::new(line_number_expected, line_number_actual);
                }
                mismatch.expected.push(str.to_vec());
                line_number_expected += 1;
            }
            diff::Result::Right(str) => {
                mismatch.actual.push(str.to_vec());
                line_number_actual += 1;
            }
            diff::Result::Both(_str, _) => {
                line_number_expected += 1;
                line_number_actual += 1;
                if !mismatch.actual.is_empty() || !mismatch.expected.is_empty() {
                    results.push(mismatch);
                    mismatch = Mismatch::new(line_number_expected, line_number_actual);
                } else {
                    mismatch.line_number_expected = line_number_expected;
                    mismatch.line_number_actual = line_number_actual;
                }
            }
        }
        if stop_early && !results.is_empty() {
            // Optimization: stop analyzing the files as soon as there are any differences
            return results;
        }
    }

    if !mismatch.actual.is_empty() || !mismatch.expected.is_empty() {
        results.push(mismatch);
    }

    results
}

pub fn diff(expected: &[u8], actual: &[u8], params: &Params) -> Vec<u8> {
    let mut output = Vec::new();
    let diff_results = make_diff(expected, actual, params);
    if params.brief && !diff_results.is_empty() {
        write!(&mut output, "\0").unwrap();
        return output;
    }
    // Like GNU diff, write the hunks last to first. Each command then refers
    // to the original line numbers, since editing later lines first leaves
    // the earlier ones where they were.
    for result in diff_results.iter().rev() {
        let line_number_expected = result.line_number_expected;
        let expected_count = result.expected.len();
        let actual_count = result.actual.len();
        match (expected_count, actual_count) {
            (0, 0) => unreachable!(),
            (0, _) => writeln!(&mut output, "{}a", line_number_expected - 1).unwrap(),
            (1, 0) => writeln!(&mut output, "{line_number_expected}d").unwrap(),
            (_, 0) => writeln!(
                &mut output,
                "{},{}d",
                line_number_expected,
                expected_count + line_number_expected - 1
            )
            .unwrap(),
            (1, _) => writeln!(&mut output, "{line_number_expected}c").unwrap(),
            _ => writeln!(
                &mut output,
                "{},{}c",
                line_number_expected,
                expected_count + line_number_expected - 1
            )
            .unwrap(),
        }
        if actual_count != 0 {
            // A line holding just "." would end the text, so like GNU diff it
            // is written as "..", the text is ended, and s/.// fixes it up.
            // Text mode is only entered again if more lines follow.
            let mut in_text = true;
            for actual in &result.actual {
                if !in_text {
                    writeln!(&mut output, "a").unwrap();
                    in_text = true;
                }
                if actual == b"." {
                    writeln!(&mut output, "..\n.\ns/.//").unwrap();
                    in_text = false;
                } else {
                    do_write_line(&mut output, actual, params.expand_tabs, params.tabsize, b"")
                        .unwrap();
                    writeln!(&mut output).unwrap();
                }
            }
            if in_text {
                writeln!(&mut output, ".").unwrap();
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use crate::utils::testcmds::ED_CMD;

    pub fn diff_w(expected: &[u8], actual: &[u8], filename: &str) -> Vec<u8> {
        let mut output = diff(expected, actual, &Params::default());
        writeln!(&mut output, "w {filename}").unwrap();
        output
    }

    #[rstest]
    #[case::changed_line(b"a\n", b"b\n", b"1c\nb\n.\n")]
    #[case::single_line_deleted(b"a\nb\n", b"a\n", b"2d\n")]
    fn test_basic(#[case] from: &[u8], #[case] to: &[u8], #[case] expected: &[u8]) {
        assert_eq!(diff(from, to, &Params::default()), expected);
    }

    #[test]
    fn test_permutations() {
        let target = "target/ed-diff/";
        // test all possible six-line files.
        let _ = std::fs::create_dir(target);
        for &a in &[0, 1, 2] {
            for &b in &[0, 1, 2] {
                for &c in &[0, 1, 2] {
                    for &d in &[0, 1, 2] {
                        for &e in &[0, 1, 2] {
                            for &f in &[0, 1, 2] {
                                use std::fs::File;
                                use std::io::Write;
                                let mut alef = Vec::new();
                                let mut bet = Vec::new();
                                alef.write_all(if a == 0 { b"a\n" } else { b"b\n" })
                                    .unwrap();
                                if a != 2 {
                                    bet.write_all(b"b\n").unwrap();
                                }
                                alef.write_all(if b == 0 { b"c\n" } else { b"d\n" })
                                    .unwrap();
                                if b != 2 {
                                    bet.write_all(b"d\n").unwrap();
                                }
                                alef.write_all(if c == 0 { b"e\n" } else { b"f\n" })
                                    .unwrap();
                                if c != 2 {
                                    bet.write_all(b"f\n").unwrap();
                                }
                                alef.write_all(if d == 0 { b"g\n" } else { b"h\n" })
                                    .unwrap();
                                if d != 2 {
                                    bet.write_all(b"h\n").unwrap();
                                }
                                alef.write_all(if e == 0 { b"i\n" } else { b"j\n" })
                                    .unwrap();
                                if e != 2 {
                                    bet.write_all(b"j\n").unwrap();
                                }
                                alef.write_all(if f == 0 { b"k\n" } else { b"l\n" })
                                    .unwrap();
                                if f != 2 {
                                    bet.write_all(b"l\n").unwrap();
                                }
                                // This test diff is intentionally reversed.
                                // We want it to turn the alef into bet.
                                let diff = diff_w(&alef, &bet, &format!("{target}/alef"));
                                File::create(format!("{target}/ab.ed"))
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(format!("{target}/alef")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(format!("{target}/bet")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                #[cfg(not(windows))] // there's no ed on windows
                                {
                                    let output = ED_CMD
                                        .new()
                                        .arg(format!("{target}/alef"))
                                        .stdin(File::open(format!("{target}/ab.ed")).unwrap())
                                        .output()
                                        .unwrap();
                                    assert!(output.status.success(), "{output:?}");
                                    //println!("{}", String::from_utf8_lossy(&output.stdout));
                                    //println!("{}", String::from_utf8_lossy(&output.stderr));
                                    let alef = std::fs::read(format!("{target}/alef")).unwrap();
                                    assert_eq!(alef, bet);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_permutations_empty_lines() {
        let target = "target/ed-diff/";
        // test all possible six-line files with missing newlines.
        let _ = std::fs::create_dir(target);
        for &a in &[0, 1, 2] {
            for &b in &[0, 1, 2] {
                for &c in &[0, 1, 2] {
                    for &d in &[0, 1, 2] {
                        for &e in &[0, 1, 2] {
                            for &f in &[0, 1, 2] {
                                use std::fs::File;
                                use std::io::Write;
                                let mut alef = Vec::new();
                                let mut bet = Vec::new();
                                alef.write_all(if a == 0 { b"\n" } else { b"b\n" }).unwrap();
                                if a != 2 {
                                    bet.write_all(b"b\n").unwrap();
                                }
                                alef.write_all(if b == 0 { b"\n" } else { b"d\n" }).unwrap();
                                if b != 2 {
                                    bet.write_all(b"d\n").unwrap();
                                }
                                alef.write_all(if c == 0 { b"\n" } else { b"f\n" }).unwrap();
                                if c != 2 {
                                    bet.write_all(b"f\n").unwrap();
                                }
                                alef.write_all(if d == 0 { b"\n" } else { b"h\n" }).unwrap();
                                if d != 2 {
                                    bet.write_all(b"h\n").unwrap();
                                }
                                alef.write_all(if e == 0 { b"\n" } else { b"j\n" }).unwrap();
                                if e != 2 {
                                    bet.write_all(b"j\n").unwrap();
                                }
                                alef.write_all(if f == 0 { b"\n" } else { b"l\n" }).unwrap();
                                if f != 2 {
                                    bet.write_all(b"l\n").unwrap();
                                }
                                // This test diff is intentionally reversed.
                                // We want it to turn the alef into bet.
                                let diff = diff_w(&alef, &bet, &format!("{target}/alef_"));
                                File::create(format!("{target}/ab_.ed"))
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(format!("{target}/alef_")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(format!("{target}/bet_")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                #[cfg(not(windows))] // there's no ed on windows
                                {
                                    let output = ED_CMD
                                        .new()
                                        .arg(format!("{target}/alef_"))
                                        .stdin(File::open(format!("{target}/ab_.ed")).unwrap())
                                        .output()
                                        .unwrap();
                                    assert!(output.status.success(), "{output:?}");
                                    //println!("{}", String::from_utf8_lossy(&output.stdout));
                                    //println!("{}", String::from_utf8_lossy(&output.stderr));
                                    let alef = std::fs::read(format!("{target}/alef_")).unwrap();
                                    assert_eq!(alef, bet);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_permutations_reverse() {
        let target = "target/ed-diff/";
        // test all possible six-line files.
        let _ = std::fs::create_dir(target);
        for &a in &[0, 1, 2] {
            for &b in &[0, 1, 2] {
                for &c in &[0, 1, 2] {
                    for &d in &[0, 1, 2] {
                        for &e in &[0, 1, 2] {
                            for &f in &[0, 1, 2] {
                                use std::fs::File;
                                use std::io::Write;
                                let mut alef = Vec::new();
                                let mut bet = Vec::new();
                                alef.write_all(if a == 0 { b"a\n" } else { b"f\n" })
                                    .unwrap();
                                if a != 2 {
                                    bet.write_all(b"a\n").unwrap();
                                }
                                alef.write_all(if b == 0 { b"b\n" } else { b"e\n" })
                                    .unwrap();
                                if b != 2 {
                                    bet.write_all(b"b\n").unwrap();
                                }
                                alef.write_all(if c == 0 { b"c\n" } else { b"d\n" })
                                    .unwrap();
                                if c != 2 {
                                    bet.write_all(b"c\n").unwrap();
                                }
                                alef.write_all(if d == 0 { b"d\n" } else { b"c\n" })
                                    .unwrap();
                                if d != 2 {
                                    bet.write_all(b"d\n").unwrap();
                                }
                                alef.write_all(if e == 0 { b"e\n" } else { b"b\n" })
                                    .unwrap();
                                if e != 2 {
                                    bet.write_all(b"e\n").unwrap();
                                }
                                alef.write_all(if f == 0 { b"f\n" } else { b"a\n" })
                                    .unwrap();
                                if f != 2 {
                                    bet.write_all(b"f\n").unwrap();
                                }
                                // This test diff is intentionally reversed.
                                // We want it to turn the alef into bet.
                                let diff = diff_w(&alef, &bet, &format!("{target}/alefr"));
                                File::create(format!("{target}/abr.ed"))
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(format!("{target}/alefr")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(format!("{target}/betr")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                #[cfg(not(windows))] // there's no ed on windows
                                {
                                    let output = ED_CMD
                                        .new()
                                        .arg(format!("{target}/alefr"))
                                        .stdin(File::open(format!("{target}/abr.ed")).unwrap())
                                        .output()
                                        .unwrap();
                                    assert!(output.status.success(), "{output:?}");
                                    //println!("{}", String::from_utf8_lossy(&output.stdout));
                                    //println!("{}", String::from_utf8_lossy(&output.stderr));
                                    let alef = std::fs::read(format!("{target}/alefr")).unwrap();
                                    assert_eq!(alef, bet);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[rstest]
    #[case::full(false, "a\nd\nc\n", "2c\nd\n.\n")]
    #[case::brief(true, "a\nd\nc\n", "\0")]
    #[case::full_identical(false, "a\nb\nc\n", "")]
    #[case::brief_identical(true, "a\nb\nc\n", "")]
    fn test_stop_early(#[case] brief: bool, #[case] to: &str, #[case] expected: &str) {
        let params = Params {
            brief,
            ..Default::default()
        };
        assert_eq!(
            diff(b"a\nb\nc\n", to.as_bytes(), &params),
            expected.as_bytes()
        );
    }
}
