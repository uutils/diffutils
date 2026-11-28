// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::ffi::OsStr;

use crate::params::Params;
use crate::utils::do_write_line;

fn write_lines(output: &mut Vec<u8>, lines: &[&[u8]], expand_tabs: bool, tabsize: usize) {
    for &line in lines {
        do_write_line(output, line, expand_tabs, tabsize, b"").expect("write to Vec is infallible");
        output.push(b'\n');
    }
}

fn write_directive(output: &mut Vec<u8>, prefix: &[u8], name: &[u8], suffix: Option<&[u8]>) {
    output.extend_from_slice(prefix);
    output.extend_from_slice(name);
    if let Some(s) = suffix {
        output.extend_from_slice(s);
    }
    output.push(b'\n');
}

fn flush_hunk(
    output: &mut Vec<u8>,
    current_expected: &mut Vec<&[u8]>,
    current_actual: &mut Vec<&[u8]>,
    ifdef_name: &[u8],
    expand_tabs: bool,
    tabsize: usize,
) -> bool {
    match (!current_expected.is_empty(), !current_actual.is_empty()) {
        (true, false) => {
            write_directive(output, b"#ifndef ", ifdef_name, None);
            write_lines(output, current_expected, expand_tabs, tabsize);
            write_directive(output, b"#endif /* ! ", ifdef_name, Some(b" */"));
        }
        (false, true) => {
            write_directive(output, b"#ifdef ", ifdef_name, None);
            write_lines(output, current_actual, expand_tabs, tabsize);
            write_directive(output, b"#endif /* ", ifdef_name, Some(b" */"));
        }
        (true, true) => {
            write_directive(output, b"#ifndef ", ifdef_name, None);
            write_lines(output, current_expected, expand_tabs, tabsize);
            write_directive(output, b"#else /* ", ifdef_name, Some(b" */"));
            write_lines(output, current_actual, expand_tabs, tabsize);
            write_directive(output, b"#endif /* ", ifdef_name, Some(b" */"));
        }
        (false, false) => return false,
    }
    current_expected.clear();
    current_actual.clear();
    true
}

/// Produces a merged output with preprocessor `#ifdef` / `#ifndef` / `#else` / `#endif` blocks.
///
/// Returns `(output_bytes, differs)` where `differs` is true if any difference was found between the files.
pub fn diff(expected: &[u8], actual: &[u8], params: &Params) -> (Vec<u8>, bool) {
    let ifdef_name = params
        .ifdef_name
        .as_deref()
        .map(OsStr::as_encoded_bytes)
        .unwrap_or(b"");
    let mut output = Vec::new();

    let mut expected_lines: Vec<&[u8]> = expected.split(|&c| c == b'\n').collect();
    let mut actual_lines: Vec<&[u8]> = actual.split(|&c| c == b'\n').collect();

    let expected_lines_count = expected_lines.len() - 1;
    let actual_lines_count = actual_lines.len() - 1;

    if expected_lines.last() == Some(&&b""[..]) {
        expected_lines.pop();
    }
    if actual_lines.last() == Some(&&b""[..]) {
        actual_lines.pop();
    }

    let mut line_number_expected = 1;
    let mut line_number_actual = 1;
    let mut differs = false;

    let mut current_expected: Vec<&[u8]> = Vec::new();
    let mut current_actual: Vec<&[u8]> = Vec::new();

    for result in diff::slice(&expected_lines, &actual_lines) {
        match result {
            diff::Result::Left(&line) => {
                current_expected.push(line);
                line_number_expected += 1;
            }
            diff::Result::Right(&line) => {
                current_actual.push(line);
                line_number_actual += 1;
            }
            diff::Result::Both(&line1, &line2) => {
                let exp_missing_nl = line_number_expected > expected_lines_count;
                let act_missing_nl = line_number_actual > actual_lines_count;
                if exp_missing_nl != act_missing_nl {
                    current_expected.push(line1);
                    current_actual.push(line2);
                } else {
                    differs |= flush_hunk(
                        &mut output,
                        &mut current_expected,
                        &mut current_actual,
                        ifdef_name,
                        params.expand_tabs,
                        params.tabsize,
                    );
                    write_lines(&mut output, &[line1], params.expand_tabs, params.tabsize);
                }
                line_number_expected += 1;
                line_number_actual += 1;
            }
        }
    }

    differs |= flush_hunk(
        &mut output,
        &mut current_expected,
        &mut current_actual,
        ifdef_name,
        params.expand_tabs,
        params.tabsize,
    );

    (output, differs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn test_identical() {
        let params = Params {
            ifdef_name: Some(OsString::from("ZZZ")),
            ..Default::default()
        };
        let (out, differs) = diff(b"1\n2\n3\n", b"1\n2\n3\n", &params);
        assert!(!differs);
        assert_eq!(out, b"1\n2\n3\n");
    }

    #[test]
    fn test_deleted_lines() {
        let params = Params {
            ifdef_name: Some(OsString::from("ZZZ")),
            ..Default::default()
        };
        let (out, differs) = diff(b"1\n2\n3\n", b"1\n3\n", &params);
        assert!(differs);
        assert_eq!(out, b"1\n#ifndef ZZZ\n2\n#endif /* ! ZZZ */\n3\n");
    }

    #[test]
    fn test_added_lines() {
        let params = Params {
            ifdef_name: Some(OsString::from("ZZZ")),
            ..Default::default()
        };
        let (out, differs) = diff(b"1\n3\n", b"1\n2\n3\n", &params);
        assert!(differs);
        assert_eq!(out, b"1\n#ifdef ZZZ\n2\n#endif /* ZZZ */\n3\n");
    }

    #[test]
    fn test_changed_lines() {
        let params = Params {
            ifdef_name: Some(OsString::from("ZZZ")),
            ..Default::default()
        };
        let (out, differs) = diff(b"1\n2\n3\n", b"1\nfoo\n3\n", &params);
        assert!(differs);
        assert_eq!(
            out,
            b"1\n#ifndef ZZZ\n2\n#else /* ZZZ */\nfoo\n#endif /* ZZZ */\n3\n"
        );
    }
}
