// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::io::Write;

use crate::params::Params;
use crate::utils::do_write_line;
use crate::duwriteln as writeln;
use crate::split_at_eol;

#[derive(Debug, PartialEq)]
struct Mismatch {
    pub line_number_expected: usize,
    pub line_number_actual: usize,
    pub expected: Vec<Vec<u8>>,
    pub actual: Vec<Vec<u8>>,
    pub expected_missing_nl: bool,
    pub actual_missing_nl: bool,
}

impl Mismatch {
    fn new(line_number_expected: usize, line_number_actual: usize) -> Mismatch {
        Mismatch {
            line_number_expected,
            line_number_actual,
            expected: Vec::new(),
            actual: Vec::new(),
            expected_missing_nl: false,
            actual_missing_nl: false,
        }
    }
}

// Produces a diff between the expected output and actual output.
fn make_diff(expected: &[u8], actual: &[u8], stop_early: bool) -> Vec<Mismatch> {
    let mut line_number_expected = 1;
    let mut line_number_actual = 1;
    let mut results = Vec::new();
    let mut mismatch = Mismatch::new(line_number_expected, line_number_actual);

    let mut expected_lines: Vec<&[u8]> = split_at_eol!(expected);
    let mut actual_lines: Vec<&[u8]> = split_at_eol!(actual);

    debug_assert_eq!(b"".split(|&c| c == b'\n').count(), 1);
    // ^ means that underflow here is impossible
    let expected_lines_count = expected_lines.len() - 1;
    let actual_lines_count = actual_lines.len() - 1;

    if expected_lines.last() == Some(&&b""[..]) {
        expected_lines.pop();
    }

    if actual_lines.last() == Some(&&b""[..]) {
        actual_lines.pop();
    }

    for result in diff::slice(&expected_lines, &actual_lines) {
        match result {
            diff::Result::Left(str) => {
                if !mismatch.actual.is_empty() && !mismatch.actual_missing_nl {
                    results.push(mismatch);
                    mismatch = Mismatch::new(line_number_expected, line_number_actual);
                }
                mismatch.expected.push(str.to_vec());
                mismatch.expected_missing_nl = line_number_expected > expected_lines_count;
                line_number_expected += 1;
            }
            diff::Result::Right(str) => {
                mismatch.actual.push(str.to_vec());
                mismatch.actual_missing_nl = line_number_actual > actual_lines_count;
                line_number_actual += 1;
            }
            diff::Result::Both(str, _) => {
                match (
                    line_number_expected > expected_lines_count,
                    line_number_actual > actual_lines_count,
                ) {
                    (true, false) => {
                        line_number_expected += 1;
                        line_number_actual += 1;
                        mismatch.expected.push(str.to_vec());
                        mismatch.expected_missing_nl = true;
                        mismatch.actual.push(str.to_vec());
                    }
                    (false, true) => {
                        line_number_expected += 1;
                        line_number_actual += 1;
                        mismatch.actual.push(str.to_vec());
                        mismatch.actual_missing_nl = true;
                        mismatch.expected.push(str.to_vec());
                    }
                    (true, true) | (false, false) => {
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

#[must_use]
pub fn diff(expected: &[u8], actual: &[u8], params: &Params) -> Vec<u8> {
    // See https://www.gnu.org/software/diffutils/manual/html_node/Detailed-Normal.html
    // for details on the syntax of the normal format.
    let mut output = Vec::new();
    let diff_results = make_diff(expected, actual, params.brief);
    if params.brief && !diff_results.is_empty() {
        write!(&mut output, "\0").unwrap();
        return output;
    }
    for result in diff_results {
        let line_number_expected = result.line_number_expected;
        let line_number_actual = result.line_number_actual;
        let expected_count = result.expected.len();
        let actual_count = result.actual.len();
        match (expected_count, actual_count) {
            (0, 0) => unreachable!(),
            (0, _) => writeln!(
                // 'a' stands for "Add lines"
                &mut output,
                "{}a{},{}",
                line_number_expected - 1,
                line_number_actual,
                line_number_actual + actual_count - 1
            )
            .unwrap(),
            (_, 0) => writeln!(
                // 'd' stands for "Delete lines"
                &mut output,
                "{},{}d{}",
                line_number_expected,
                expected_count + line_number_expected - 1,
                line_number_actual - 1
            )
            .unwrap(),
            (1, 1) => writeln!(
                // 'c' stands for "Change lines"
                // exactly one line replaced by one line
                &mut output,
                "{}c{}",
                line_number_expected,
                line_number_actual
            )
            .unwrap(),
            (1, _) => writeln!(
                // one line replaced by multiple lines
                &mut output,
                "{}c{},{}",
                line_number_expected,
                line_number_actual,
                actual_count + line_number_actual - 1
            )
            .unwrap(),
            (_, 1) => writeln!(
                // multiple lines replaced by one line
                &mut output,
                "{},{}c{}",
                line_number_expected,
                expected_count + line_number_expected - 1,
                line_number_actual
            )
            .unwrap(),
            _ => writeln!(
                // general case: multiple lines replaced by multiple lines
                &mut output,
                "{},{}c{},{}",
                line_number_expected,
                expected_count + line_number_expected - 1,
                line_number_actual,
                actual_count + line_number_actual - 1
            )
            .unwrap(),
        }
        for expected in &result.expected {
            write!(&mut output, "< ").unwrap();
            do_write_line(&mut output, expected, params.expand_tabs, params.tabsize).unwrap();
            writeln!(&mut output).unwrap();
        }
        if result.expected_missing_nl {
            writeln!(&mut output, r"\ No newline at end of file").unwrap();
        }
        if expected_count != 0 && actual_count != 0 {
            writeln!(&mut output, "---").unwrap();
        }
        for actual in &result.actual {
            write!(&mut output, "> ").unwrap();
            do_write_line(&mut output, actual, params.expand_tabs, params.tabsize).unwrap();
            writeln!(&mut output).unwrap();
        }
        if result.actual_missing_nl {
            writeln!(&mut output, r"\ No newline at end of file").unwrap();
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use crate::duwriteln as writeln;
    use crate::multiwriteln;
    use super::*;
    use pretty_assertions::assert_eq;

    #[cfg(windows)]
    macro_rules! pop_eol {
        ($buf:expr) => ($buf.pop(); $buf.pop());
    }

    #[cfg(not(windows))]
    macro_rules! pop_eol {
    ($buf:expr) => ($buf.pop());
    }

    #[test]
    fn test_basic() {
        let mut a = Vec::new();
        writeln!(a, "a").unwrap();
        let mut b = Vec::new();
        writeln!(b, "b").unwrap();
        let diff = diff(&a, &b, &Params::default());
        let mut expected = Vec::new();
        multiwriteln!(expected, "1c1", "< a", "---", "> b");
        assert_eq!(diff, expected);
    }

    #[test]
    fn test_permutations() {
        let target = "target/normal-diff/";
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
                                writeln!(alef, "{}", if a == 0 { "a" } else { "b" }).unwrap();
                                if a != 2 {
                                    writeln!(bet, "b").unwrap();
                                }
                                writeln!(alef, "{}", if b == 0 { "c" } else { "d" }).unwrap();
                                if b != 2 {
                                    writeln!(bet, "d").unwrap();
                                }
                                writeln!(alef, "{}", if c == 0 { "e" } else { "f" }).unwrap();
                                if c != 2 {
                                    writeln!(bet, "f").unwrap();
                                }
                                writeln!(alef, "{}", if d == 0 { "g" } else { "h" }).unwrap();
                                if d != 2 {
                                    writeln!(bet, "h").unwrap();
                                }
                                writeln!(alef, "{}", if e == 0 { "i" } else { "j" }).unwrap();
                                if e != 2 {
                                    writeln!(bet, "j").unwrap();
                                }
                                writeln!(alef, "{}", if f == 0 { "k" } else { "l" }).unwrap();
                                if f != 2 {
                                    writeln!(bet, "l").unwrap();
                                }
                                // This test diff is intentionally reversed.
                                // We want it to turn the alef into bet.
                                let diff = diff(&alef, &bet, &Params::default());
                                File::create(&format!("{target}/ab.diff"))
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(&format!("{target}/alef")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(&format!("{target}/bet")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                let output = Command::new("patch")
                                    .arg("-p0")
                                    .arg(&format!("{target}/alef"))
                                    .stdin(File::open(&format!("{target}/ab.diff")).unwrap())
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
    fn test_permutations_missing_line_endinga() {
        let target = "target/normal-diff/";
        // test all possible six-line files with missing newlines.
        let _ = std::fs::create_dir(target);
        for &a in &[0, 1, 2] {
            for &b in &[0, 1, 2] {
                for &c in &[0, 1, 2] {
                    for &d in &[0, 1, 2] {
                        for &e in &[0, 1, 2] {
                            for &f in &[0, 1, 2] {
                                for &g in &[0, 1, 2] {
                                    use std::fs::{self, File};
                                    use std::io::Write;
                                    use std::process::Command;
                                    let mut alef = Vec::new();
                                    let mut bet = Vec::new();
                                    writeln!(alef, "{}", if a == 0 { "a" } else { "b" }).unwrap();
                                    if a != 2 {
                                        writeln!(bet, "b").unwrap();
                                    }
                                    writeln!(alef, "{}", if b == 0 { "c" } else { "d" }).unwrap();
                                    if b != 2 {
                                        writeln!(bet, "d").unwrap();
                                    }
                                    writeln!(alef, "{}", if c == 0 { "e" } else { "f" }).unwrap();
                                    if c != 2 {
                                        writeln!(bet, "f").unwrap();
                                    }
                                    writeln!(alef, "{}", if d == 0 { "g" } else { "h" }).unwrap();
                                    if d != 2 {
                                        writeln!(bet, "h").unwrap();
                                    }
                                    writeln!(alef, "{}", if e == 0 { "i" } else { "j" }).unwrap();
                                    if e != 2 {
                                        writeln!(bet, "j").unwrap();
                                    }
                                    writeln!(alef, "{}", if f == 0 { "k" } else { "l" }).unwrap();
                                    if f != 2 {
                                        writeln!(bet, "l").unwrap();
                                    }
                                    match g {
                                        0 => {
                                            pop_eol!(alef);
                                        }
                                        1 => {
                                            pop_eol!(bet);
                                        }
                                        2 => {
                                            pop_eol!(alef);
                                            pop_eol!(bet);
                                        }
                                        _ => unreachable!(),
                                    }
                                    // This test diff is intentionally reversed.
                                    // We want it to turn the alef into bet.
                                    let diff = diff(&alef, &bet, &Params::default());
                                    File::create(&format!("{target}/abn.diff"))
                                        .unwrap()
                                        .write_all(&diff)
                                        .unwrap();
                                    let mut fa = File::create(&format!("{target}/alefn")).unwrap();
                                    fa.write_all(&alef[..]).unwrap();
                                    let mut fb = File::create(&format!("{target}/betn")).unwrap();
                                    fb.write_all(&bet[..]).unwrap();
                                    let _ = fa;
                                    let _ = fb;
                                    let output = Command::new("patch")
                                        .arg("-p0")
                                        .arg("--normal")
                                        .arg(&format!("{target}/alefn"))
                                        .stdin(File::open(&format!("{target}/abn.diff")).unwrap())
                                        .output()
                                        .unwrap();
                                    assert!(output.status.success(), "{:?}", output);
                                    //println!("{}", String::from_utf8_lossy(&output.stdout));
                                    //println!("{}", String::from_utf8_lossy(&output.stderr));
                                    let alef = fs::read(&format!("{target}/alefn")).unwrap();
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
        let target = "target/normal-diff/";
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
                                writeln!(alef, "{}", if a == 0 { "" } else { "b" }).unwrap();
                                if a != 2 {
                                    writeln!(bet, "b").unwrap();
                                }
                                writeln!(alef, "{}", if b == 0 { "" } else { "d" }).unwrap();
                                if b != 2 {
                                    writeln!(bet, "d").unwrap();
                                }
                                writeln!(alef, "{}", if c == 0 { "" } else { "f" }).unwrap();
                                if c != 2 {
                                    writeln!(bet, "f").unwrap();
                                }
                                writeln!(alef, "{}", if d == 0 { "" } else { "h" }).unwrap();
                                if d != 2 {
                                    writeln!(bet, "h").unwrap();
                                }
                                writeln!(alef, "{}", if e == 0 { "" } else { "j" }).unwrap();
                                if e != 2 {
                                    writeln!(bet, "j").unwrap();
                                }
                                writeln!(alef, "{}", if f == 0 { "" } else { "l" }).unwrap();
                                if f != 2 {
                                    writeln!(bet, "l").unwrap();
                                }
                                // This test diff is intentionally reversed.
                                // We want it to turn the alef into bet.
                                let diff = diff(&alef, &bet, &Params::default());
                                File::create(&format!("{target}/ab_.diff"))
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(&format!("{target}/alef_")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(&format!("{target}/bet_")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                let output = Command::new("patch")
                                    .arg("-p0")
                                    .arg(&format!("{target}/alef_"))
                                    .stdin(File::open(&format!("{target}/ab_.diff")).unwrap())
                                    .output()
                                    .unwrap();
                                assert!(output.status.success(), "{:?}", output);
                                //println!("{}", String::from_utf8_lossy(&output.stdout));
                                //println!("{}", String::from_utf8_lossy(&output.stderr));
                                let alef = fs::read(&format!("{target}/alef_")).unwrap();
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
        let target = "target/normal-diff/";
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
                                writeln!(alef, "{}", if a == 0 { "a" } else { "f" }).unwrap();
                                if a != 2 {
                                    writeln!(bet, "a").unwrap();
                                }
                                writeln!(alef, "{}", if b == 0 { "b" } else { "e" }).unwrap();
                                if b != 2 {
                                    writeln!(bet, "b").unwrap();
                                }
                                writeln!(alef, "{}", if c == 0 { "c" } else { "d" }).unwrap();
                                if c != 2 {
                                    writeln!(bet, "c").unwrap();
                                }
                                writeln!(alef, "{}", if d == 0 { "d" } else { "c" }).unwrap();
                                if d != 2 {
                                    writeln!(bet, "d").unwrap();
                                }
                                writeln!(alef, "{}", if e == 0 { "e" } else { "b" }).unwrap();
                                if e != 2 {
                                    writeln!(bet, "e").unwrap();
                                }
                                writeln!(alef, "{}", if f == 0 { "f" } else { "a" }).unwrap();
                                if f != 2 {
                                    writeln!(bet, "f").unwrap();
                                }
                                // This test diff is intentionally reversed.
                                // We want it to turn the alef into bet.
                                let diff = diff(&alef, &bet, &Params::default());
                                File::create(&format!("{target}/abr.diff"))
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(&format!("{target}/alefr")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(&format!("{target}/betr")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                let output = Command::new("patch")
                                    .arg("-p0")
                                    .arg(&format!("{target}/alefr"))
                                    .stdin(File::open(&format!("{target}/abr.diff")).unwrap())
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
        let from = ["a", "b", "c"].join("\n");
        let to = ["a", "d", "c"].join("\n");

        let diff_full = diff(from.as_bytes(), to.as_bytes(), &Params::default());
        let expected_full = ["2c2", "< b", "---", "> d", ""].join("\n");
        assert_eq!(diff_full, expected_full.as_bytes());

        let diff_brief = diff(
            from.as_bytes(),
            to.as_bytes(),
            &Params {
                brief: true,
                ..Default::default()
            },
        );
        let expected_brief = "\0".as_bytes();
        assert_eq!(diff_brief, expected_brief);

        let nodiff_full = diff(from.as_bytes(), from.as_bytes(), &Params::default());
        assert!(nodiff_full.is_empty());

        let nodiff_brief = diff(
            from.as_bytes(),
            from.as_bytes(),
            &Params {
                brief: true,
                ..Default::default()
            },
        );
        assert!(nodiff_brief.is_empty());
    }
}
