// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::collections::VecDeque;
use std::io::Write;

#[derive(Debug, PartialEq)]
pub enum DiffLine {
    Context(Vec<u8>),
    Change(Vec<u8>),
    Add(Vec<u8>),
}

#[derive(Debug, PartialEq)]
struct Mismatch {
    pub line_number_expected: usize,
    pub line_number_actual: usize,
    pub expected: Vec<DiffLine>,
    pub actual: Vec<DiffLine>,
    pub expected_missing_nl: bool,
    pub actual_missing_nl: bool,
    pub expected_all_context: bool,
    pub actual_all_context: bool,
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
            expected_all_context: false,
            actual_all_context: false,
        }
    }
}

// Produces a diff between the expected output and actual output.
fn make_diff(expected: &[u8], actual: &[u8], context_size: usize) -> Vec<Mismatch> {
    let mut line_number_expected = 1;
    let mut line_number_actual = 1;
    let mut context_queue: VecDeque<&[u8]> = VecDeque::with_capacity(context_size);
    let mut lines_since_mismatch = context_size + 1;
    let mut results = Vec::new();
    let mut mismatch = Mismatch::new(0, 0);

    let mut expected_lines: Vec<&[u8]> = expected.split(|&c| c == b'\n').collect();
    let mut actual_lines: Vec<&[u8]> = actual.split(|&c| c == b'\n').collect();

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

    // Rust only allows allocations to grow to isize::MAX, and this is bigger than that.
    let mut expected_lines_change_idx: usize = !0;

    for result in diff::slice(&expected_lines, &actual_lines) {
        match result {
            diff::Result::Left(str) => {
                if lines_since_mismatch > context_size && lines_since_mismatch > 0 {
                    results.push(mismatch);
                    mismatch = Mismatch::new(
                        line_number_expected - context_queue.len(),
                        line_number_actual - context_queue.len(),
                    );
                }

                while let Some(line) = context_queue.pop_front() {
                    mismatch.expected.push(DiffLine::Context(line.to_vec()));
                    mismatch.actual.push(DiffLine::Context(line.to_vec()));
                }

                expected_lines_change_idx = mismatch.expected.len();
                mismatch.expected.push(DiffLine::Add(str.to_vec()));
                if line_number_expected > expected_lines_count {
                    mismatch.expected_missing_nl = true;
                }
                line_number_expected += 1;
                lines_since_mismatch = 0;
            }
            diff::Result::Right(str) => {
                if lines_since_mismatch > context_size && lines_since_mismatch > 0 {
                    results.push(mismatch);
                    mismatch = Mismatch::new(
                        line_number_expected - context_queue.len(),
                        line_number_actual - context_queue.len(),
                    );
                    expected_lines_change_idx = !0;
                }

                while let Some(line) = context_queue.pop_front() {
                    mismatch.expected.push(DiffLine::Context(line.to_vec()));
                    mismatch.actual.push(DiffLine::Context(line.to_vec()));
                }

                if let Some(DiffLine::Add(content)) =
                    mismatch.expected.get_mut(expected_lines_change_idx)
                {
                    let content = std::mem::take(content);
                    mismatch.expected[expected_lines_change_idx] = DiffLine::Change(content);
                    expected_lines_change_idx = expected_lines_change_idx.wrapping_sub(1); // if 0, becomes !0
                    mismatch.actual.push(DiffLine::Change(str.to_vec()));
                } else {
                    mismatch.actual.push(DiffLine::Add(str.to_vec()));
                }
                if line_number_actual > actual_lines_count {
                    mismatch.actual_missing_nl = true;
                }
                line_number_actual += 1;
                lines_since_mismatch = 0;
            }
            diff::Result::Both(str, _) => {
                expected_lines_change_idx = !0;
                // if one of them is missing a newline and the other isn't, then they don't actually match
                if (line_number_actual > actual_lines_count)
                    && (line_number_expected > expected_lines_count)
                {
                    if context_queue.len() < context_size {
                        while let Some(line) = context_queue.pop_front() {
                            mismatch.expected.push(DiffLine::Context(line.to_vec()));
                            mismatch.actual.push(DiffLine::Context(line.to_vec()));
                        }
                        if lines_since_mismatch < context_size {
                            mismatch.expected.push(DiffLine::Context(str.to_vec()));
                            mismatch.actual.push(DiffLine::Context(str.to_vec()));
                            mismatch.expected_missing_nl = true;
                            mismatch.actual_missing_nl = true;
                        }
                    }
                    lines_since_mismatch = 0;
                } else if line_number_actual > actual_lines_count {
                    if lines_since_mismatch >= context_size && lines_since_mismatch > 0 {
                        results.push(mismatch);
                        mismatch = Mismatch::new(
                            line_number_expected - context_queue.len(),
                            line_number_actual - context_queue.len(),
                        );
                    }
                    while let Some(line) = context_queue.pop_front() {
                        mismatch.expected.push(DiffLine::Context(line.to_vec()));
                        mismatch.actual.push(DiffLine::Context(line.to_vec()));
                    }
                    mismatch.expected.push(DiffLine::Change(str.to_vec()));
                    mismatch.actual.push(DiffLine::Change(str.to_vec()));
                    mismatch.actual_missing_nl = true;
                    lines_since_mismatch = 0;
                } else if line_number_expected > expected_lines_count {
                    if lines_since_mismatch >= context_size && lines_since_mismatch > 0 {
                        results.push(mismatch);
                        mismatch = Mismatch::new(
                            line_number_expected - context_queue.len(),
                            line_number_actual - context_queue.len(),
                        );
                    }
                    while let Some(line) = context_queue.pop_front() {
                        mismatch.expected.push(DiffLine::Context(line.to_vec()));
                        mismatch.actual.push(DiffLine::Context(line.to_vec()));
                    }
                    mismatch.expected.push(DiffLine::Change(str.to_vec()));
                    mismatch.expected_missing_nl = true;
                    mismatch.actual.push(DiffLine::Change(str.to_vec()));
                    lines_since_mismatch = 0;
                } else {
                    debug_assert!(context_queue.len() <= context_size);
                    if context_queue.len() >= context_size {
                        let _ = context_queue.pop_front();
                    }
                    if lines_since_mismatch < context_size {
                        mismatch.expected.push(DiffLine::Context(str.to_vec()));
                        mismatch.actual.push(DiffLine::Context(str.to_vec()));
                    } else if context_size > 0 {
                        context_queue.push_back(str);
                    }
                    lines_since_mismatch += 1;
                }
                line_number_expected += 1;
                line_number_actual += 1;
            }
        }
    }

    results.push(mismatch);
    results.remove(0);

    if results.is_empty() && expected_lines_count != actual_lines_count {
        let mut mismatch = Mismatch::new(expected_lines.len(), actual_lines.len());
        // empty diff and only expected lines has a missing line at end
        if expected_lines_count != expected_lines.len() {
            mismatch.expected.push(DiffLine::Change(
                expected_lines
                    .pop()
                    .expect("can't be empty; produced by split()")
                    .to_vec(),
            ));
            mismatch.expected_missing_nl = true;
            mismatch.actual.push(DiffLine::Change(
                actual_lines
                    .pop()
                    .expect("can't be empty; produced by split()")
                    .to_vec(),
            ));
            results.push(mismatch);
        } else if actual_lines_count != actual_lines.len() {
            mismatch.expected.push(DiffLine::Change(
                expected_lines
                    .pop()
                    .expect("can't be empty; produced by split()")
                    .to_vec(),
            ));
            mismatch.actual.push(DiffLine::Change(
                actual_lines
                    .pop()
                    .expect("can't be empty; produced by split()")
                    .to_vec(),
            ));
            mismatch.actual_missing_nl = true;
            results.push(mismatch);
        }
    }

    // hunks with pure context lines get truncated to empty
    for mismatch in &mut results {
        if !mismatch
            .expected
            .iter()
            .any(|x| !matches!(&x, DiffLine::Context(_)))
        {
            mismatch.expected_all_context = true;
        }
        if !mismatch
            .actual
            .iter()
            .any(|x| !matches!(&x, DiffLine::Context(_)))
        {
            mismatch.actual_all_context = true;
        }
    }

    results
}

pub fn diff(
    expected: &[u8],
    expected_filename: &str,
    actual: &[u8],
    actual_filename: &str,
    context_size: usize,
) -> Vec<u8> {
    let mut output =
        format!("*** {}\t\n--- {}\t\n", expected_filename, actual_filename).into_bytes();
    let diff_results = make_diff(expected, actual, context_size);
    if diff_results.is_empty() {
        return Vec::new();
    };
    for result in diff_results {
        let mut line_number_expected = result.line_number_expected;
        let mut line_number_actual = result.line_number_actual;
        let mut expected_count = result.expected.len();
        let mut actual_count = result.actual.len();
        if expected_count == 0 {
            line_number_expected -= 1;
            expected_count = 1;
        }
        if actual_count == 0 {
            line_number_actual -= 1;
            actual_count = 1;
        }
        let end_line_number_expected = expected_count + line_number_expected - 1;
        let end_line_number_actual = actual_count + line_number_actual - 1;
        let exp_start = if end_line_number_expected == line_number_expected {
            String::new()
        } else {
            format!("{},", line_number_expected)
        };
        let act_start = if end_line_number_actual == line_number_actual {
            String::new()
        } else {
            format!("{},", line_number_actual)
        };
        writeln!(
            output,
            "***************\n*** {}{} ****",
            exp_start, end_line_number_expected
        )
        .expect("write to Vec is infallible");
        if !result.expected_all_context {
            for line in result.expected {
                match line {
                    DiffLine::Context(e) => {
                        write!(output, "  ").expect("write to Vec is infallible");
                        output.write_all(&e).expect("write to Vec is infallible");
                        writeln!(output).unwrap();
                    }
                    DiffLine::Change(e) => {
                        write!(output, "! ").expect("write to Vec is infallible");
                        output.write_all(&e).expect("write to Vec is infallible");
                        writeln!(output).unwrap();
                    }
                    DiffLine::Add(e) => {
                        write!(output, "- ").expect("write to Vec is infallible");
                        output.write_all(&e).expect("write to Vec is infallible");
                        writeln!(output).unwrap();
                    }
                }
            }
            if result.expected_missing_nl {
                writeln!(output, r"\ No newline at end of file")
                    .expect("write to Vec is infallible");
            }
        }
        writeln!(output, "--- {}{} ----", act_start, end_line_number_actual)
            .expect("write to Vec is infallible");
        if !result.actual_all_context {
            for line in result.actual {
                match line {
                    DiffLine::Context(e) => {
                        write!(output, "  ").expect("write to Vec is infallible");
                        output.write_all(&e).expect("write to Vec is infallible");
                        writeln!(output).unwrap();
                    }
                    DiffLine::Change(e) => {
                        write!(output, "! ").expect("write to Vec is infallible");
                        output.write_all(&e).expect("write to Vec is infallible");
                        writeln!(output).unwrap();
                    }
                    DiffLine::Add(e) => {
                        write!(output, "+ ").expect("write to Vec is infallible");
                        output.write_all(&e).expect("write to Vec is infallible");
                        writeln!(output).unwrap();
                    }
                }
            }
            if result.actual_missing_nl {
                writeln!(output, r"\ No newline at end of file")
                    .expect("write to Vec is infallible");
            }
        }
    }
    output
}

#[test]
fn test_permutations() {
    // test all possible six-line files.
    let _ = std::fs::create_dir("target");
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
                            let diff = diff(&alef, "a/alef", &bet, "target/alef", 2);
                            File::create("target/ab.diff")
                                .unwrap()
                                .write_all(&diff)
                                .unwrap();
                            let mut fa = File::create("target/alef").unwrap();
                            fa.write_all(&alef[..]).unwrap();
                            let mut fb = File::create("target/bet").unwrap();
                            fb.write_all(&bet[..]).unwrap();
                            let _ = fa;
                            let _ = fb;
                            let output = Command::new("patch")
                                .arg("-p0")
                                .arg("--context")
                                .stdin(File::open("target/ab.diff").unwrap())
                                .output()
                                .unwrap();
                            if !output.status.success() {
                                panic!("{:?}", output);
                            }
                            //println!("{}", String::from_utf8_lossy(&output.stdout));
                            //println!("{}", String::from_utf8_lossy(&output.stderr));
                            let alef = fs::read("target/alef").unwrap();
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
    // test all possible six-line files with missing newlines.
    let _ = std::fs::create_dir("target");
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
                            let diff = diff(&alef, "a/alef_", &bet, "target/alef_", 2);
                            File::create("target/ab_.diff")
                                .unwrap()
                                .write_all(&diff)
                                .unwrap();
                            let mut fa = File::create("target/alef_").unwrap();
                            fa.write_all(&alef[..]).unwrap();
                            let mut fb = File::create("target/bet_").unwrap();
                            fb.write_all(&bet[..]).unwrap();
                            let _ = fa;
                            let _ = fb;
                            let output = Command::new("patch")
                                .arg("-p0")
                                .arg("--context")
                                .stdin(File::open("target/ab_.diff").unwrap())
                                .output()
                                .unwrap();
                            if !output.status.success() {
                                panic!("{:?}", output);
                            }
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
fn test_permutations_missing_lines() {
    // test all possible six-line files.
    let _ = std::fs::create_dir("target");
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
                            alef.write_all(if a == 0 { b"a\n" } else { b"" }).unwrap();
                            if a != 2 {
                                bet.write_all(b"b\n").unwrap();
                            }
                            alef.write_all(if b == 0 { b"c\n" } else { b"" }).unwrap();
                            if b != 2 {
                                bet.write_all(b"d\n").unwrap();
                            }
                            alef.write_all(if c == 0 { b"e\n" } else { b"" }).unwrap();
                            if c != 2 {
                                bet.write_all(b"f\n").unwrap();
                            }
                            alef.write_all(if d == 0 { b"g\n" } else { b"" }).unwrap();
                            if d != 2 {
                                bet.write_all(b"h\n").unwrap();
                            }
                            alef.write_all(if e == 0 { b"i\n" } else { b"" }).unwrap();
                            if e != 2 {
                                bet.write_all(b"j\n").unwrap();
                            }
                            alef.write_all(if f == 0 { b"k\n" } else { b"" }).unwrap();
                            if f != 2 {
                                bet.write_all(b"l\n").unwrap();
                            }
                            if alef.is_empty() && bet.is_empty() {
                                continue;
                            };
                            // This test diff is intentionally reversed.
                            // We want it to turn the alef into bet.
                            let diff = diff(&alef, "a/alefx", &bet, "target/alefx", 2);
                            File::create("target/abx.diff")
                                .unwrap()
                                .write_all(&diff)
                                .unwrap();
                            let mut fa = File::create("target/alefx").unwrap();
                            fa.write_all(&alef[..]).unwrap();
                            let mut fb = File::create("target/betx").unwrap();
                            fb.write_all(&bet[..]).unwrap();
                            let _ = fa;
                            let _ = fb;
                            let output = Command::new("patch")
                                .arg("-p0")
                                .arg("--context")
                                .stdin(File::open("target/abx.diff").unwrap())
                                .output()
                                .unwrap();
                            if !output.status.success() {
                                panic!("{:?}", output);
                            }
                            //println!("{}", String::from_utf8_lossy(&output.stdout));
                            //println!("{}", String::from_utf8_lossy(&output.stderr));
                            let alef = fs::read("target/alefx").unwrap();
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
    // test all possible six-line files.
    let _ = std::fs::create_dir("target");
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
                            let diff = diff(&alef, "a/alefr", &bet, "target/alefr", 2);
                            File::create("target/abr.diff")
                                .unwrap()
                                .write_all(&diff)
                                .unwrap();
                            let mut fa = File::create("target/alefr").unwrap();
                            fa.write_all(&alef[..]).unwrap();
                            let mut fb = File::create("target/betr").unwrap();
                            fb.write_all(&bet[..]).unwrap();
                            let _ = fa;
                            let _ = fb;
                            let output = Command::new("patch")
                                .arg("-p0")
                                .arg("--context")
                                .stdin(File::open("target/abr.diff").unwrap())
                                .output()
                                .unwrap();
                            if !output.status.success() {
                                panic!("{:?}", output);
                            }
                            //println!("{}", String::from_utf8_lossy(&output.stdout));
                            //println!("{}", String::from_utf8_lossy(&output.stderr));
                            let alef = fs::read("target/alefr").unwrap();
                            assert_eq!(alef, bet);
                        }
                    }
                }
            }
        }
    }
}
