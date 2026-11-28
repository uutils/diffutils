// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::collections::VecDeque;
use std::io::Write;

use crate::diff_engine::{diff_slices, Change};
use crate::params::Params;
use crate::utils::{comparison_lines, do_write_line, line_prefix};
use crate::utils::{
    get_modification_time, is_posix_locale, write_file_header, FunctionLineFinder, TimestampStyle,
};

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

/// A run of changed lines with no common line between them.
struct ChangeBlock {
    expected_start: usize,
    actual_start: usize,
    deletes: bool,
    inserts: bool,
}

impl ChangeBlock {
    fn new(mismatch: &Mismatch) -> Self {
        Self {
            expected_start: mismatch.expected.len(),
            actual_start: mismatch.actual.len(),
            deletes: false,
            inserts: false,
        }
    }

    /// Opens a block at the end of `mismatch` if none is open, and records
    /// whether it deletes or inserts lines.
    fn mark(block: &mut Option<Self>, mismatch: &Mismatch, deletes: bool, inserts: bool) {
        let block = block.get_or_insert_with(|| Self::new(mismatch));
        block.deletes |= deletes;
        block.inserts |= inserts;
    }

    fn close(block: &mut Option<Self>, mismatch: &mut Mismatch) {
        if let Some(block) = block.take() {
            block.finish(mismatch);
        }
    }

    /// Like GNU diff, a block that both deletes and inserts lines marks all of
    /// them as changed (`!`), not only as many as pair up.
    fn finish(self, mismatch: &mut Mismatch) {
        if !(self.deletes && self.inserts) {
            return;
        }
        let expected = mismatch.expected[self.expected_start..].iter_mut();
        let actual = mismatch.actual[self.actual_start..].iter_mut();
        for line in expected.chain(actual) {
            if let DiffLine::Add(content) = line {
                *line = DiffLine::Change(std::mem::take(content));
            }
        }
    }
}

// Produces a diff between the expected output and actual output.
fn make_diff(expected: &[u8], actual: &[u8], params: &Params) -> Vec<Mismatch> {
    let context_size = params.context_count;
    let stop_early = params.brief;
    let mut line_number_expected = 1;
    let mut line_number_actual = 1;
    let mut context_queue: VecDeque<&[u8]> = VecDeque::with_capacity(context_size);
    // Like GNU diff, keep changes in one hunk while at most twice the context
    // size of unchanged lines separates them.
    let max_gap = 2 * context_size;
    let mut lines_since_mismatch = max_gap + 1;
    let mut results = Vec::new();
    let mut mismatch = Mismatch::new(0, 0);

    let mut expected_lines = comparison_lines(expected, params);
    let mut actual_lines = comparison_lines(actual, params);

    debug_assert_eq!(b"".split(|&c| c == b'\n').count(), 1);
    // ^ means that underflow here is impossible
    let expected_lines_count = expected_lines.len() - 1;
    let actual_lines_count = actual_lines.len() - 1;

    if expected_lines.last().is_some_and(|l| l.raw.is_empty()) {
        expected_lines.pop();
    }

    if actual_lines.last().is_some_and(|l| l.raw.is_empty()) {
        actual_lines.pop();
    }

    let mut block: Option<ChangeBlock> = None;

    for result in diff_slices(&expected_lines, &actual_lines) {
        match result {
            Change::Left(str) => {
                if lines_since_mismatch > max_gap {
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

                ChangeBlock::mark(&mut block, &mismatch, true, false);
                mismatch.expected.push(DiffLine::Add(str.to_vec()));
                if line_number_expected > expected_lines_count {
                    mismatch.expected_missing_nl = true;
                }
                line_number_expected += 1;
                lines_since_mismatch = 0;
            }
            Change::Right(str) => {
                if lines_since_mismatch > max_gap {
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

                ChangeBlock::mark(&mut block, &mismatch, false, true);
                mismatch.actual.push(DiffLine::Add(str.to_vec()));
                if line_number_actual > actual_lines_count {
                    mismatch.actual_missing_nl = true;
                }
                line_number_actual += 1;
                lines_since_mismatch = 0;
            }
            Change::Both(str, _) => {
                // if one of them is missing a newline and the other isn't, then they don't actually match
                if (line_number_actual > actual_lines_count)
                    && (line_number_expected > expected_lines_count)
                {
                    ChangeBlock::close(&mut block, &mut mismatch);
                    // this is the last line: lines still in the queue are past
                    // the trailing context and there is no next hunk to lead into
                    if lines_since_mismatch < context_size {
                        mismatch.expected.push(DiffLine::Context(str.to_vec()));
                        mismatch.actual.push(DiffLine::Context(str.to_vec()));
                        mismatch.expected_missing_nl = true;
                        mismatch.actual_missing_nl = true;
                    }
                } else if line_number_actual > actual_lines_count {
                    if lines_since_mismatch > max_gap {
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
                    ChangeBlock::mark(&mut block, &mismatch, true, true);
                    mismatch.expected.push(DiffLine::Change(str.to_vec()));
                    mismatch.actual.push(DiffLine::Change(str.to_vec()));
                    mismatch.actual_missing_nl = true;
                    lines_since_mismatch = 0;
                } else if line_number_expected > expected_lines_count {
                    if lines_since_mismatch > max_gap {
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
                    ChangeBlock::mark(&mut block, &mismatch, true, true);
                    mismatch.expected.push(DiffLine::Change(str.to_vec()));
                    mismatch.expected_missing_nl = true;
                    mismatch.actual.push(DiffLine::Change(str.to_vec()));
                    lines_since_mismatch = 0;
                } else {
                    ChangeBlock::close(&mut block, &mut mismatch);
                    debug_assert!(context_queue.len() <= context_size);
                    if context_queue.len() >= context_size {
                        let _ = context_queue.pop_front();
                    }
                    if lines_since_mismatch < context_size {
                        mismatch.expected.push(DiffLine::Context(str.to_vec()));
                        mismatch.actual.push(DiffLine::Context(str.to_vec()));
                    } else if context_size > 0 {
                        context_queue.push_back(str.raw);
                    }
                    lines_since_mismatch += 1;
                }
                line_number_expected += 1;
                line_number_actual += 1;
            }
        }
        if stop_early && !results.is_empty() {
            // Optimization: stop analyzing the files as soon as there are any differences
            return results;
        }
    }

    if let Some(block) = block {
        block.finish(&mut mismatch);
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

#[must_use]
pub fn diff(expected: &[u8], actual: &[u8], params: &Params) -> Vec<u8> {
    // The context format keeps the traditional timestamp in the POSIX locale.
    let style = if is_posix_locale("LC_TIME") {
        TimestampStyle::Traditional
    } else {
        TimestampStyle::Iso
    };
    let from_modified_time =
        get_modification_time(&params.from.to_string_lossy(), style, params.from_missing);
    let to_modified_time =
        get_modification_time(&params.to.to_string_lossy(), style, params.to_missing);
    let mut output = Vec::new();
    write_file_header(
        &mut output,
        "***",
        &params.from,
        params.from_label.as_deref(),
        &from_modified_time,
    );
    write_file_header(
        &mut output,
        "---",
        &params.to,
        params.to_label.as_deref(),
        &to_modified_time,
    );
    let diff_results = make_diff(expected, actual, params);
    if diff_results.is_empty() {
        return Vec::new();
    }
    if params.brief {
        return output;
    }
    let mut function_lines = FunctionLineFinder::new(expected, &params.function_line_patterns);
    for result in diff_results {
        let function_line = function_lines
            .as_mut()
            .and_then(|finder| finder.find(result.line_number_expected));
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
            format!("{line_number_expected},")
        };
        let act_start = if end_line_number_actual == line_number_actual {
            String::new()
        } else {
            format!("{line_number_actual},")
        };
        output.extend_from_slice(b"***************");
        if let Some(function_line) = function_line {
            output.push(b' ');
            output.extend_from_slice(&function_line);
        }
        writeln!(output, "\n*** {exp_start}{end_line_number_expected} ****")
            .expect("write to Vec is infallible");
        if !result.expected_all_context {
            for line in result.expected {
                match line {
                    DiffLine::Context(e) => {
                        do_write_line(
                            &mut output,
                            &e,
                            params.expand_tabs,
                            params.tabsize,
                            line_prefix(b"  ", &e, params),
                        )
                        .expect("write to Vec is infallible");
                        writeln!(output).unwrap();
                    }
                    DiffLine::Change(e) => {
                        do_write_line(
                            &mut output,
                            &e,
                            params.expand_tabs,
                            params.tabsize,
                            line_prefix(b"! ", &e, params),
                        )
                        .expect("write to Vec is infallible");
                        writeln!(output).unwrap();
                    }
                    DiffLine::Add(e) => {
                        do_write_line(
                            &mut output,
                            &e,
                            params.expand_tabs,
                            params.tabsize,
                            line_prefix(b"- ", &e, params),
                        )
                        .expect("write to Vec is infallible");
                        writeln!(output).unwrap();
                    }
                }
            }
            if result.expected_missing_nl {
                writeln!(output, r"\ No newline at end of file")
                    .expect("write to Vec is infallible");
            }
        }
        writeln!(output, "--- {act_start}{end_line_number_actual} ----")
            .expect("write to Vec is infallible");
        if !result.actual_all_context {
            for line in result.actual {
                match line {
                    DiffLine::Context(e) => {
                        do_write_line(
                            &mut output,
                            &e,
                            params.expand_tabs,
                            params.tabsize,
                            line_prefix(b"  ", &e, params),
                        )
                        .expect("write to Vec is infallible");
                        writeln!(output).unwrap();
                    }
                    DiffLine::Change(e) => {
                        do_write_line(
                            &mut output,
                            &e,
                            params.expand_tabs,
                            params.tabsize,
                            line_prefix(b"! ", &e, params),
                        )
                        .expect("write to Vec is infallible");
                        writeln!(output).unwrap();
                    }
                    DiffLine::Add(e) => {
                        do_write_line(
                            &mut output,
                            &e,
                            params.expand_tabs,
                            params.tabsize,
                            line_prefix(b"+ ", &e, params),
                        )
                        .expect("write to Vec is infallible");
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use crate::utils::testcmds::PATCH_CMD;

    #[test]
    fn test_permutations() {
        // test all possible six-line files.
        let target = "target/context-diff/";
        let _ = std::fs::create_dir(target);
        for &a in &[0, 1, 2] {
            for &b in &[0, 1, 2] {
                for &c in &[0, 1, 2] {
                    for &d in &[0, 1, 2] {
                        for &e in &[0, 1, 2] {
                            for &f in &[0, 1, 2] {
                                use std::fs::{self, File};
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
                                let patched = &format!("{target}/alef");
                                let diff = diff(
                                    &alef,
                                    &bet,
                                    &Params {
                                        from: patched.into(),
                                        to: patched.into(),
                                        context_count: 2,
                                        ..Default::default()
                                    },
                                );
                                File::create(format!("{target}/ab.diff"))
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(format!("{target}/alef")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(format!("{target}/bet")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                let output = PATCH_CMD
                                    .new()
                                    .arg("-p0")
                                    .arg("--context")
                                    .stdin(File::open(format!("{target}/ab.diff")).unwrap())
                                    .output()
                                    .unwrap();
                                assert!(output.status.success(), "{output:?}");
                                //println!("{}", String::from_utf8_lossy(&output.stdout));
                                //println!("{}", String::from_utf8_lossy(&output.stderr));
                                let alef = fs::read(format!("{target}/alef")).unwrap();
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
        let target = "target/context-diff/";
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
                                let patched = &format!("{target}/alef_");
                                let diff = diff(
                                    &alef,
                                    &bet,
                                    &Params {
                                        from: patched.into(),
                                        to: patched.into(),
                                        context_count: 2,
                                        ..Default::default()
                                    },
                                );
                                File::create(format!("{target}/ab_.diff"))
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(format!("{target}/alef_")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(format!("{target}/bet_")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                let output = PATCH_CMD
                                    .new()
                                    .arg("-p0")
                                    .arg("--context")
                                    .stdin(File::open(format!("{target}/ab_.diff")).unwrap())
                                    .output()
                                    .unwrap();
                                assert!(output.status.success(), "{output:?}");
                                //println!("{}", String::from_utf8_lossy(&output.stdout));
                                //println!("{}", String::from_utf8_lossy(&output.stderr));
                                let alef = fs::read(format!("{target}/alef_")).unwrap();
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
        let target = "target/context-diff/";
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
                                let patched = &format!("{target}/alefx");
                                let diff = diff(
                                    &alef,
                                    &bet,
                                    &Params {
                                        from: patched.into(),
                                        to: patched.into(),
                                        context_count: 2,
                                        ..Default::default()
                                    },
                                );
                                File::create(format!("{target}/abx.diff"))
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(format!("{target}/alefx")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(format!("{target}/betx")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                let output = PATCH_CMD
                                    .new()
                                    .arg("-p0")
                                    .arg("--context")
                                    .stdin(File::open(format!("{target}/abx.diff")).unwrap())
                                    .output()
                                    .unwrap();
                                assert!(output.status.success(), "{output:?}");
                                //println!("{}", String::from_utf8_lossy(&output.stdout));
                                //println!("{}", String::from_utf8_lossy(&output.stderr));
                                let alef = fs::read(format!("{target}/alefx")).unwrap();
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
        let target = "target/context-diff/";
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
                                let alefr_path = &format!("{target}/alefr");
                                let diff = diff(
                                    &alef,
                                    &bet,
                                    &Params {
                                        from: alefr_path.into(),
                                        to: alefr_path.into(),
                                        context_count: 2,
                                        ..Default::default()
                                    },
                                );
                                File::create(format!("{target}/abr.diff"))
                                    .unwrap()
                                    .write_all(&diff)
                                    .unwrap();
                                let mut fa = File::create(format!("{target}/alefr")).unwrap();
                                fa.write_all(&alef[..]).unwrap();
                                let mut fb = File::create(format!("{target}/betr")).unwrap();
                                fb.write_all(&bet[..]).unwrap();
                                let _ = fa;
                                let _ = fb;
                                let output = PATCH_CMD
                                    .new()
                                    .arg("-p0")
                                    .arg("--context")
                                    .stdin(File::open(format!("{target}/abr.diff")).unwrap())
                                    .output()
                                    .unwrap();
                                assert!(output.status.success(), "{output:?}");
                                //println!("{}", String::from_utf8_lossy(&output.stdout));
                                //println!("{}", String::from_utf8_lossy(&output.stderr));
                                let alef = fs::read(format!("{target}/alefr")).unwrap();
                                assert_eq!(alef, bet);
                            }
                        }
                    }
                }
            }
        }
    }

    #[rstest]
    #[case::full(
        false,
        "a\nd\nc\n",
        &[
            "*** foo\tTIMESTAMP",
            "--- bar\tTIMESTAMP",
            "***************",
            "*** 1,3 ****",
            "  a",
            "! b",
            "  c",
            "--- 1,3 ----",
            "  a",
            "! d",
            "  c",
            "",
        ]
    )]
    #[case::brief(true, "a\nd\nc\n", &["*** foo\tTIMESTAMP", "--- bar\tTIMESTAMP", ""])]
    #[case::full_identical(false, "a\nb\nc\n", &[])]
    #[case::brief_identical(true, "a\nb\nc\n", &[])]
    fn test_stop_early(#[case] brief: bool, #[case] to: &str, #[case] expected: &[&str]) {
        use crate::assert_diff_eq;

        let params = Params {
            from: "foo".into(),
            to: "bar".into(),
            brief,
            ..Default::default()
        };
        let output = diff(b"a\nb\nc\n", to.as_bytes(), &params);
        assert_diff_eq!(output, expected.join("\n"));
    }

    #[rstest]
    // Two lines replaced by one: both old lines are changed, not one deleted.
    #[case::two_lines_replaced_by_one(
        "a\nb\nc\nd\n",
        "a\nx\nd\n",
        3,
        &[
            "*** 1,4 ****",
            "  a",
            "! b",
            "! c",
            "  d",
            "--- 1,3 ----",
            "  a",
            "! x",
            "  d",
        ]
    )]
    // One line replaced by three: all three new lines are changed.
    #[case::one_line_replaced_by_three(
        "a\nb\n",
        "a\nx\ny\nz\n",
        3,
        &["*** 1,2 ****", "  a", "! b", "--- 1,4 ----", "  a", "! x", "! y", "! z"]
    )]
    // Changes separated by up to twice the context size share one hunk.
    #[case::changes_share_hunk(
        "a\nb\nc\nd\ne\nf\n",
        "a\nB\nc\nd\nE\nf\n",
        1,
        &[
            "*** 1,6 ****",
            "  a",
            "! b",
            "  c",
            "  d",
            "! e",
            "  f",
            "--- 1,6 ----",
            "  a",
            "! B",
            "  c",
            "  d",
            "! E",
            "  f",
        ]
    )]
    // One more unchanged line between them splits the hunk.
    #[case::changes_split_hunks(
        "a\nb\nc\nd\ne\nf\ng\n",
        "a\nB\nc\nd\ne\nF\ng\n",
        1,
        &[
            "*** 1,3 ****",
            "  a",
            "! b",
            "  c",
            "--- 1,3 ----",
            "  a",
            "! B",
            "  c",
            "***************",
            "*** 5,7 ****",
            "  e",
            "! f",
            "  g",
            "--- 5,7 ----",
            "  e",
            "! F",
            "  g",
        ]
    )]
    // An insertion before a nearby change gives the two sides different
    // line counts; the hunks must still be merged (issue #268).
    #[case::insertion_before_change(
        "milk\neggs\nbread\nbutter\napples\n",
        "milk\neggs\nyogurt\nbread\nhoney\napples\n",
        1,
        &[
            "*** 2,5 ****",
            "  eggs",
            "  bread",
            "! butter",
            "  apples",
            "--- 2,6 ----",
            "  eggs",
            "+ yogurt",
            "  bread",
            "! honey",
            "  apples",
        ]
    )]
    fn test_hunks(
        #[case] from: &str,
        #[case] to: &str,
        #[case] context_count: usize,
        #[case] hunks: &[&str],
    ) {
        use crate::assert_diff_eq;

        let params = Params {
            from: "foo".into(),
            to: "bar".into(),
            context_count,
            ..Default::default()
        };
        let expected = format!(
            "*** foo\tTIMESTAMP\n--- bar\tTIMESTAMP\n***************\n{}\n",
            hunks.join("\n")
        );
        let output = diff(from.as_bytes(), to.as_bytes(), &params);
        assert_diff_eq!(output, expected);
    }

    #[test]
    fn test_function_lines() {
        use crate::assert_diff_eq;

        let from = [
            "(defun area",
            "  (* w h))",
            "(defun volume",
            "  (* w h d))",
            "",
        ]
        .join("\n");
        let to = from.replace("d))", "depth))");
        let params = Params {
            from: "old".into(),
            to: "new".into(),
            context_count: 0,
            function_line_patterns: vec![crate::bre::Bre::new("^(").unwrap()],
            ..Default::default()
        };
        let expected = [
            "*** old\tTIMESTAMP",
            "--- new\tTIMESTAMP",
            "*************** (defun volume",
            "*** 4 ****",
            "!   (* w h d))",
            "--- 4 ----",
            "!   (* w h depth))",
            "",
        ]
        .join("\n");
        let output = diff(from.as_bytes(), to.as_bytes(), &params);
        assert_diff_eq!(output, expected);
    }
}
