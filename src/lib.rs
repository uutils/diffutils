use std::collections::VecDeque;
use std::io::Write;

#[derive(Debug, PartialEq)]
pub enum DiffLine {
    Context(Vec<u8>),
    Expected(Vec<u8>),
    Resulting(Vec<u8>),
    MissingNL,
}

#[derive(Debug, PartialEq)]
struct Mismatch {
    pub line_number: u32,
    pub line_number_resulting: u32,
    pub lines: Vec<DiffLine>,
}

impl Mismatch {
    fn new(line_number: u32, line_number_resulting: u32) -> Mismatch {
        Mismatch {
            line_number,
            line_number_resulting,
            lines: Vec::new(),
        }
    }
}

// Produces a diff between the expected output and actual output.
fn make_diff(expected: &[u8], actual: &[u8], context_size: usize) -> Vec<Mismatch> {
    let mut line_number = 1;
    let mut line_number_resulting = 1;
    let mut context_queue: VecDeque<&[u8]> = VecDeque::with_capacity(context_size);
    let mut lines_since_mismatch = context_size + 1;
    let mut results = Vec::new();
    let mut mismatch = Mismatch::new(0, 0);

    let mut expected_lines: Vec<&[u8]> = expected.split(|&c| c == b'\n').collect();
    let mut actual_lines: Vec<&[u8]> = actual.split(|&c| c == b'\n').collect();

    let expected_lines_count = (expected_lines.len() as u32).wrapping_sub(1);
    let actual_lines_count = (actual_lines.len() as u32).wrapping_sub(1);

    if expected_lines.last() == Some(&&b""[..]) {
        expected_lines.pop();
    }

    if actual_lines.last() == Some(&&b""[..]) {
        actual_lines.pop();
    }

    for result in diff::slice(&expected_lines, &actual_lines) {
        match result {
            diff::Result::Left(str) => {
                if lines_since_mismatch >= context_size && lines_since_mismatch > 0 {
                    results.push(mismatch);
                    mismatch = Mismatch::new(
                        line_number - context_queue.len() as u32,
                        line_number_resulting - context_queue.len() as u32,
                    );
                }

                while let Some(line) = context_queue.pop_front() {
                    mismatch.lines.push(DiffLine::Context(line.to_vec()));
                }

                if mismatch.lines.last() == Some(&DiffLine::MissingNL) {
                    mismatch.lines.pop();
                    match mismatch.lines.pop() {
                        Some(DiffLine::Resulting(res)) => {
                            mismatch.lines.push(DiffLine::Expected(str.to_vec()));
                            if line_number > expected_lines_count {
                                mismatch.lines.push(DiffLine::MissingNL)
                            }
                            mismatch.lines.push(DiffLine::Resulting(res));
                            mismatch.lines.push(DiffLine::MissingNL);
                        }
                        _ => unreachable!(),
                    }
                } else {
                    mismatch.lines.push(DiffLine::Expected(str.to_vec()));
                    if line_number > expected_lines_count {
                        mismatch.lines.push(DiffLine::MissingNL)
                    }
                }
                line_number += 1;
                lines_since_mismatch = 0;
            }
            diff::Result::Right(str) => {
                if lines_since_mismatch >= context_size && lines_since_mismatch > 0 {
                    results.push(mismatch);
                    mismatch = Mismatch::new(
                        line_number - context_queue.len() as u32,
                        line_number_resulting - context_queue.len() as u32,
                    );
                }

                while let Some(line) = context_queue.pop_front() {
                    assert!(mismatch.lines.last() != Some(&DiffLine::MissingNL));
                    mismatch.lines.push(DiffLine::Context(line.to_vec()));
                }

                if mismatch.lines.last() == Some(&DiffLine::MissingNL) {
                    mismatch.lines.pop();
                    match mismatch.lines.pop() {
                        Some(DiffLine::Expected(exp)) => {
                            mismatch.lines.push(DiffLine::Expected(exp));
                            mismatch.lines.push(DiffLine::MissingNL);
                            mismatch.lines.push(DiffLine::Resulting(str.to_vec()));
                        }
                        _ => unreachable!(),
                    }
                } else {
                    mismatch.lines.push(DiffLine::Resulting(str.to_vec()));
                }
                if line_number_resulting > actual_lines_count {
                    mismatch.lines.push(DiffLine::MissingNL)
                }
                line_number_resulting += 1;
                lines_since_mismatch = 0;
            }
            diff::Result::Both(str, _) => {
                if (line_number_resulting > actual_lines_count)
                    || (line_number > expected_lines_count)
                {
                    // if one of them is missing a newline and the other isn't, then they don't actually match
                    if lines_since_mismatch >= context_size
                        && lines_since_mismatch > 0
                        && (line_number_resulting > actual_lines_count)
                            != (line_number > expected_lines_count)
                    {
                        results.push(mismatch);
                        mismatch = Mismatch::new(
                            line_number - context_queue.len() as u32,
                            line_number_resulting - context_queue.len() as u32,
                        );
                    }
                    while let Some(line) = context_queue.pop_front() {
                        assert!(mismatch.lines.last() != Some(&DiffLine::MissingNL));
                        mismatch.lines.push(DiffLine::Context(line.to_vec()));
                    }
                    lines_since_mismatch = 0;
                    if line_number_resulting > actual_lines_count
                        && line_number > expected_lines_count
                    {
                        mismatch.lines.push(DiffLine::Context(str.to_vec()));
                        mismatch.lines.push(DiffLine::MissingNL);
                    } else if line_number > expected_lines_count {
                        mismatch.lines.push(DiffLine::Expected(str.to_vec()));
                        mismatch.lines.push(DiffLine::MissingNL);
                        mismatch.lines.push(DiffLine::Resulting(str.to_vec()));
                    } else if line_number_resulting > actual_lines_count {
                        mismatch.lines.push(DiffLine::Expected(str.to_vec()));
                        mismatch.lines.push(DiffLine::Resulting(str.to_vec()));
                        mismatch.lines.push(DiffLine::MissingNL);
                    }
                } else {
                    if context_queue.len() >= context_size {
                        let _ = context_queue.pop_front();
                    }
                    if lines_since_mismatch < context_size {
                        mismatch.lines.push(DiffLine::Context(str.to_vec()));
                    } else if context_size > 0 {
                        context_queue.push_back(str);
                    }
                    lines_since_mismatch += 1;
                }

                line_number += 1;
                line_number_resulting += 1;
            }
        }
    }

    results.push(mismatch);
    results.remove(0);

    if results.len() == 0 && expected_lines_count != actual_lines_count {
        let mut mismatch = Mismatch::new(expected_lines.len() as u32, actual_lines.len() as u32);
        // empty diff and only expected lines has a missing line at end
        if expected_lines_count != expected_lines.len() as u32 {
            mismatch
                .lines
                .push(DiffLine::Expected(expected_lines.pop().unwrap().to_vec()));
            mismatch.lines.push(DiffLine::MissingNL);
            mismatch
                .lines
                .push(DiffLine::Resulting(actual_lines.pop().unwrap().to_vec()));
            results.push(mismatch);
        } else if actual_lines_count != actual_lines.len() as u32 {
            mismatch
                .lines
                .push(DiffLine::Expected(expected_lines.pop().unwrap().to_vec()));
            mismatch
                .lines
                .push(DiffLine::Resulting(actual_lines.pop().unwrap().to_vec()));
            mismatch.lines.push(DiffLine::MissingNL);
            results.push(mismatch);
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
        format!("--- {}\t\n+++ {}\t\n", expected_filename, actual_filename).into_bytes();
    let diff_results = make_diff(expected, actual, context_size);
    if diff_results.len() == 0 {
        return Vec::new();
    };
    for result in diff_results {
        let line_number = result.line_number;
        let line_number_resulting = result.line_number_resulting;
        let mut expected_count = 0;
        let mut resulting_count = 0;
        for line in &result.lines {
            match line {
                DiffLine::Expected(_) => {
                    expected_count += 1;
                }
                DiffLine::Context(_) => {
                    expected_count += 1;
                    resulting_count += 1;
                }
                DiffLine::Resulting(_) => {
                    resulting_count += 1;
                }
                DiffLine::MissingNL => {}
            }
        }
        writeln!(
            output,
            "@@ -{},{} +{},{} @@",
            line_number, expected_count, line_number_resulting, resulting_count
        )
        .unwrap();
        for line in result.lines {
            match line {
                DiffLine::Expected(e) => {
                    write!(output, "-").unwrap();
                    output.write_all(&e).unwrap();
                    writeln!(output).unwrap();
                }
                DiffLine::Context(c) => {
                    write!(output, " ").unwrap();
                    output.write_all(&c).unwrap();
                    writeln!(output).unwrap();
                }
                DiffLine::Resulting(r) => {
                    write!(output, "+",).unwrap();
                    output.write_all(&r).unwrap();
                    writeln!(output).unwrap();
                }
                DiffLine::MissingNL => {
                    writeln!(output, r"\ No newline at end of file").unwrap();
                }
            }
        }
    }
    output
}

#[test]
fn test_permutations() {
    // test all possible six-line files.
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
fn test_permutations_missing_line_ending() {
    // test all possible six-line files with missing newlines.
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
                                match g {
                                    0 => {
                                        alef.pop();
                                    }
                                    1 => {
                                        bet.pop();
                                    }
                                    2 => {
                                        alef.pop();
                                        bet.pop();
                                    }
                                    _ => unreachable!(),
                                }
                                // This test diff is intentionally reversed.
                                // We want it to turn the alef into bet.
                                let diff = diff(&alef, "a/alefn", &bet, "target/alefn", 2);
                                File::create("target/abn.diff")
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create("target/alefn").unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create("target/betn").unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                let output = Command::new("patch")
                                    .arg("-p0")
                                    .stdin(File::open("target/abn.diff").unwrap())
                                    .output()
                                    .unwrap();
                                if !output.status.success() {
                                    panic!("{:?}", output);
                                }
                                //println!("{}", String::from_utf8_lossy(&output.stdout));
                                //println!("{}", String::from_utf8_lossy(&output.stderr));
                                let alef = fs::read("target/alefn").unwrap();
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
    // test all possible six-line files with missing newlines.
    for &a in &[0, 1, 2] {
        for &b in &[0, 1, 2] {
            for &c in &[0, 1, 2] {
                for &d in &[0, 1, 2] {
                    for &e in &[0, 1, 2] {
                        for &f in &[0, 1, 2] {
                            for &g in &[0, 1, 2, 3] {
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
                                match g {
                                    0 => {
                                        alef.pop();
                                    }
                                    1 => {
                                        bet.pop();
                                    }
                                    2 => {
                                        alef.pop();
                                        bet.pop();
                                    }
                                    3 => {}
                                    _ => unreachable!(),
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
}

#[test]
fn test_permutations_missing_lines() {
    // test all possible six-line files.
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
                            alef.write_all(if a == 0 { b"a\n" } else { b"f\n" }).unwrap();
                            if a != 2 {
                                bet.write_all(b"a\n").unwrap();
                            }
                            alef.write_all(if b == 0 { b"b\n" } else { b"e\n" }).unwrap();
                            if b != 2 {
                                bet.write_all(b"b\n").unwrap();
                            }
                            alef.write_all(if c == 0 { b"c\n" } else { b"d\n" }).unwrap();
                            if c != 2 {
                                bet.write_all(b"c\n").unwrap();
                            }
                            alef.write_all(if d == 0 { b"d\n" } else { b"c\n" }).unwrap();
                            if d != 2 {
                                bet.write_all(b"d\n").unwrap();
                            }
                            alef.write_all(if e == 0 { b"e\n" } else { b"b\n" }).unwrap();
                            if e != 2 {
                                bet.write_all(b"e\n").unwrap();
                            }
                            alef.write_all(if f == 0 { b"f\n" } else { b"a\n" }).unwrap();
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
