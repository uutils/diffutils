// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::io::Write;

use crate::params::Params;
use crate::utils::do_write_line;

/// Produces a merged output with preprocessor `#ifdef` / `#ifndef` / `#else` / `#endif` blocks.
///
/// Returns `(output_bytes, differs)` where `differs` is true if any difference was found between the files.
pub fn diff(expected: &[u8], actual: &[u8], params: &Params) -> (Vec<u8>, bool) {
    let ifdef_name = params.ifdef_name.as_deref().unwrap_or("");
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

    let mut current_expected: Vec<Vec<u8>> = Vec::new();
    let mut current_actual: Vec<Vec<u8>> = Vec::new();

    let flush = |output: &mut Vec<u8>,
                 current_expected: &mut Vec<Vec<u8>>,
                 current_actual: &mut Vec<Vec<u8>>,
                 differs: &mut bool| {
        if current_expected.is_empty() && current_actual.is_empty() {
            return;
        }
        *differs = true;
        match (!current_expected.is_empty(), !current_actual.is_empty()) {
            (true, false) => {
                writeln!(output, "#ifndef {ifdef_name}").unwrap();
                for line in current_expected.drain(..) {
                    do_write_line(output, &line, params.expand_tabs, params.tabsize).unwrap();
                    output.push(b'\n');
                }
                writeln!(output, "#endif /* ! {ifdef_name} */").unwrap();
            }
            (false, true) => {
                writeln!(output, "#ifdef {ifdef_name}").unwrap();
                for line in current_actual.drain(..) {
                    do_write_line(output, &line, params.expand_tabs, params.tabsize).unwrap();
                    output.push(b'\n');
                }
                writeln!(output, "#endif /* {ifdef_name} */").unwrap();
            }
            (true, true) => {
                writeln!(output, "#ifndef {ifdef_name}").unwrap();
                for line in current_expected.drain(..) {
                    do_write_line(output, &line, params.expand_tabs, params.tabsize).unwrap();
                    output.push(b'\n');
                }
                writeln!(output, "#else /* {ifdef_name} */").unwrap();
                for line in current_actual.drain(..) {
                    do_write_line(output, &line, params.expand_tabs, params.tabsize).unwrap();
                    output.push(b'\n');
                }
                writeln!(output, "#endif /* {ifdef_name} */").unwrap();
            }
            (false, false) => unreachable!(),
        }
    };

    for result in diff::slice(&expected_lines, &actual_lines) {
        match result {
            diff::Result::Left(str) => {
                current_expected.push(str.to_vec());
                line_number_expected += 1;
            }
            diff::Result::Right(str) => {
                current_actual.push(str.to_vec());
                line_number_actual += 1;
            }
            diff::Result::Both(str1, str2) => {
                let exp_missing_nl = line_number_expected > expected_lines_count;
                let act_missing_nl = line_number_actual > actual_lines_count;
                if exp_missing_nl != act_missing_nl {
                    current_expected.push(str1.to_vec());
                    current_actual.push(str2.to_vec());
                } else {
                    flush(
                        &mut output,
                        &mut current_expected,
                        &mut current_actual,
                        &mut differs,
                    );
                    do_write_line(&mut output, str1, params.expand_tabs, params.tabsize).unwrap();
                    output.push(b'\n');
                }
                line_number_expected += 1;
                line_number_actual += 1;
            }
        }
    }

    flush(
        &mut output,
        &mut current_expected,
        &mut current_actual,
        &mut differs,
    );

    (output, differs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical() {
        let params = Params {
            ifdef_name: Some("ZZZ".to_string()),
            ..Default::default()
        };
        let (out, differs) = diff(b"1\n2\n3\n", b"1\n2\n3\n", &params);
        assert!(!differs);
        assert_eq!(out, b"1\n2\n3\n");
    }

    #[test]
    fn test_deleted_lines() {
        let params = Params {
            ifdef_name: Some("ZZZ".to_string()),
            ..Default::default()
        };
        let (out, differs) = diff(b"1\n2\n3\n", b"1\n3\n", &params);
        assert!(differs);
        assert_eq!(out, b"1\n#ifndef ZZZ\n2\n#endif /* ! ZZZ */\n3\n");
    }

    #[test]
    fn test_added_lines() {
        let params = Params {
            ifdef_name: Some("ZZZ".to_string()),
            ..Default::default()
        };
        let (out, differs) = diff(b"1\n3\n", b"1\n2\n3\n", &params);
        assert!(differs);
        assert_eq!(out, b"1\n#ifdef ZZZ\n2\n#endif /* ZZZ */\n3\n");
    }

    #[test]
    fn test_changed_lines() {
        let params = Params {
            ifdef_name: Some("ZZZ".to_string()),
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
