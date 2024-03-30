// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::io::Write;

use crate::utils::do_write_line;

#[derive(Debug, PartialEq)]
struct Mismatch {
    pub line_number_expected: usize,
    pub line_number_actual: usize,
    pub expected: Vec<Vec<u8>>,
    pub actual: Vec<Vec<u8>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DiffError {
    MissingNL,
}

impl std::fmt::Display for DiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        std::fmt::Display::fmt("No newline at end of file", f)
    }
}

impl From<DiffError> for String {
    fn from(_: DiffError) -> String {
        "No newline at end of file".into()
    }
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
fn make_diff(expected: &[u8], actual: &[u8], stop_early: bool) -> Result<Vec<Mismatch>, DiffError> {
    let mut line_number_expected = 1;
    let mut line_number_actual = 1;
    let mut results = Vec::new();
    let mut mismatch = Mismatch::new(line_number_expected, line_number_actual);

    let mut expected_lines: Vec<&[u8]> = expected.split(|&c| c == b'\n').collect();
    let mut actual_lines: Vec<&[u8]> = actual.split(|&c| c == b'\n').collect();

    debug_assert_eq!(b"".split(|&c| c == b'\n').count(), 1);
    // ^ means that underflow here is impossible
    let _expected_lines_count = expected_lines.len() - 1;
    let _actual_lines_count = actual_lines.len() - 1;

    if expected_lines.last() == Some(&&b""[..]) {
        expected_lines.pop();
    } else {
        return Err(DiffError::MissingNL);
    }

    if actual_lines.last() == Some(&&b""[..]) {
        actual_lines.pop();
    } else {
        return Err(DiffError::MissingNL);
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
            return Ok(results);
        }
    }

    if !mismatch.actual.is_empty() || !mismatch.expected.is_empty() {
        results.push(mismatch);
    }

    Ok(results)
}

pub fn diff(
    expected: &[u8],
    actual: &[u8],
    stop_early: bool,
    expand_tabs: bool,
    tabsize: usize,
) -> Result<Vec<u8>, DiffError> {
    let mut output = Vec::new();
    let diff_results = make_diff(expected, actual, stop_early)?;
    if stop_early && !diff_results.is_empty() {
        write!(&mut output, "\0").unwrap();
        return Ok(output);
    }
    let mut lines_offset = 0;
    for result in diff_results {
        let line_number_expected: isize = result.line_number_expected as isize + lines_offset;
        let _line_number_actual: isize = result.line_number_actual as isize + lines_offset;
        let expected_count: isize = result.expected.len() as isize;
        let actual_count: isize = result.actual.len() as isize;
        match (expected_count, actual_count) {
            (0, 0) => unreachable!(),
            (0, _) => writeln!(&mut output, "{}a", line_number_expected - 1).unwrap(),
            (_, 0) => writeln!(
                &mut output,
                "{},{}d",
                line_number_expected,
                expected_count + line_number_expected - 1
            )
            .unwrap(),
            (1, _) => writeln!(&mut output, "{}c", line_number_expected).unwrap(),
            _ => writeln!(
                &mut output,
                "{},{}c",
                line_number_expected,
                expected_count + line_number_expected - 1
            )
            .unwrap(),
        }
        lines_offset += actual_count - expected_count;
        if actual_count != 0 {
            for actual in &result.actual {
                if actual == b"." {
                    writeln!(&mut output, "..\n.\ns/.//\na").unwrap();
                } else {
                    do_write_line(&mut output, actual, expand_tabs, tabsize).unwrap();
                    writeln!(&mut output).unwrap();
                }
            }
            writeln!(&mut output, ".").unwrap();
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    pub fn diff_w(expected: &[u8], actual: &[u8], filename: &str) -> Result<Vec<u8>, DiffError> {
        let mut output = diff(expected, actual, false, false, 8)?;
        writeln!(&mut output, "w {filename}").unwrap();
        Ok(output)
    }

    #[test]
    fn test_basic() {
        let from = b"a\n";
        let to = b"b\n";
        let diff = diff(from, to, false, false, 8).unwrap();
        let expected = ["1c", "b", ".", ""].join("\n");
        assert_eq!(diff, expected.as_bytes());
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
                                use std::fs::{self, File};
                                use std::io::Write;
                                use std::process::Command;
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
                                let diff = diff_w(&alef, &bet, &format!("{target}/alef")).unwrap();
                                File::create("target/ab.ed")
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(&format!("{target}/alef")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(&format!("{target}/bet")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                let output = Command::new("ed")
                                    .arg(&format!("{target}/alef"))
                                    .stdin(File::open("target/ab.ed").unwrap())
                                    .output()
                                    .unwrap();
                                assert!(output.status.success(), "{:?}", output);
                                //println!("{}", String::from_utf8_lossy(&output.stdout));
                                //println!("{}", String::from_utf8_lossy(&output.stderr));
                                let alef = fs::read(&format!("{target}/alef")).unwrap();
                                assert_eq!(alef, bet);
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
                                use std::fs::{self, File};
                                use std::io::Write;
                                use std::process::Command;
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
                                let diff = diff_w(&alef, &bet, "target/alef_").unwrap();
                                File::create("target/ab_.ed")
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create("target/alef_").unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(&format!("{target}/bet_")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                let output = Command::new("ed")
                                    .arg("target/alef_")
                                    .stdin(File::open("target/ab_.ed").unwrap())
                                    .output()
                                    .unwrap();
                                assert!(output.status.success(), "{:?}", output);
                                //println!("{}", String::from_utf8_lossy(&output.stdout));
                                //println!("{}", String::from_utf8_lossy(&output.stderr));
                                let alef = fs::read("target/alef_").unwrap();
                                assert_eq!(alef, bet);
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
                                use std::fs::{self, File};
                                use std::io::Write;
                                use std::process::Command;
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
                                let diff = diff_w(&alef, &bet, &format!("{target}/alefr")).unwrap();
                                File::create("target/abr.ed")
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(&format!("{target}/alefr")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(&format!("{target}/betr")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                let output = Command::new("ed")
                                    .arg(&format!("{target}/alefr"))
                                    .stdin(File::open("target/abr.ed").unwrap())
                                    .output()
                                    .unwrap();
                                assert!(output.status.success(), "{:?}", output);
                                //println!("{}", String::from_utf8_lossy(&output.stdout));
                                //println!("{}", String::from_utf8_lossy(&output.stderr));
                                let alef = fs::read(&format!("{target}/alefr")).unwrap();
                                assert_eq!(alef, bet);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_stop_early() {
        let from = ["a", "b", "c", ""].join("\n");
        let to = ["a", "d", "c", ""].join("\n");

        let diff_full = diff(from.as_bytes(), to.as_bytes(), false, false, 8).unwrap();
        let expected_full = ["2c", "d", ".", ""].join("\n");
        assert_eq!(diff_full, expected_full.as_bytes());

        let diff_brief = diff(from.as_bytes(), to.as_bytes(), true, false, 8).unwrap();
        let expected_brief = "\0".as_bytes();
        assert_eq!(diff_brief, expected_brief);

        let nodiff_full = diff(from.as_bytes(), from.as_bytes(), false, false, 8).unwrap();
        assert!(nodiff_full.is_empty());

        let nodiff_brief = diff(from.as_bytes(), from.as_bytes(), true, false, 8).unwrap();
        assert!(nodiff_brief.is_empty());
    }
}
