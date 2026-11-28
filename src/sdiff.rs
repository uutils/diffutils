// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::params::Params;
use crate::side_diff;
use crate::utils::{is_binary, read_file_contents, report_failure_to_read_input_file, Outcome};
use std::env::ArgsOs;
use std::ffi::OsString;
use std::io::{stdout, Write};
use std::iter::Peekable;
use std::process::ExitCode;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum SdiffError {
    #[error("Usage: {0} <from> <to>")]
    Usage(String),
    #[error("option '{0}' requires an argument")]
    MissingArgument(String),
    #[error("invalid width '{0}'")]
    InvalidWidth(String),
    #[error("invalid tabsize '{0}'")]
    InvalidTabSize(String),
    #[error("unrecognized option '{0}'")]
    UnrecognizedOption(String),
}

fn parse_width(val: &str) -> Result<usize, SdiffError> {
    match val.parse::<usize>() {
        Ok(w) if w >= 7 => Ok(w),
        _ => Err(SdiffError::InvalidWidth(val.to_string())),
    }
}

fn parse_tabsize(val: &str) -> Result<usize, SdiffError> {
    match val.parse::<usize>() {
        Ok(num) if num != 0 && num <= crate::params::MAX_TABSIZE => Ok(num),
        _ => Err(SdiffError::InvalidTabSize(val.to_string())),
    }
}

/// parses command line arguments for `sdiff`.
/// this is necessary since the options differ for `diff` and `sdiff` in GNU
/// for example: `-w` sets the columns in `sdiff` but in `diff` it ignores whitespace
/// https://www.gnu.org/software/diffutils/manual/html_node/diff-Options.html
/// https://www.gnu.org/software/diffutils/manual/html_node/sdiff-Options.html
pub fn parse_params<I: Iterator<Item = OsString>>(
    mut opts: Peekable<I>,
) -> Result<Params, SdiffError> {
    let executable = opts
        .next()
        .ok_or_else(|| SdiffError::Usage("sdiff".to_string()))?;
    let mut params = Params {
        executable,
        ..Default::default()
    };

    let mut operands = Vec::new();

    while let Some(param) = opts.next() {
        let param_str = param.to_string_lossy();
        if param == "--" {
            operands.extend(opts);
            break;
        }

        if param == "-s" || param == "--suppress-common-lines" {
            params.suppress_common_lines = true;
            continue;
        }

        if param == "-l" || param == "--left-column" {
            params.left_column = true;
            continue;
        }

        if param == "-t" || param == "--expand-tabs" {
            params.expand_tabs = true;
            continue;
        }

        if param == "-a" || param == "--text" {
            params.text = true;
            continue;
        }

        if param == "-w" || param == "--width" {
            let val = opts
                .next()
                .ok_or_else(|| SdiffError::MissingArgument(param_str.to_string()))?;
            params.width = parse_width(&val.to_string_lossy())?;
            continue;
        }

        if let Some(rest) = param_str.strip_prefix("--width=") {
            params.width = parse_width(rest)?;
            continue;
        }

        if param_str.starts_with("-w") && param_str.len() > 2 {
            params.width = parse_width(&param_str[2..])?;
            continue;
        }

        if param == "--tabsize" {
            let val = opts
                .next()
                .ok_or_else(|| SdiffError::MissingArgument(param_str.to_string()))?;
            params.tabsize = parse_tabsize(&val.to_string_lossy())?;
            continue;
        }

        if let Some(rest) = param_str.strip_prefix("--tabsize=") {
            params.tabsize = parse_tabsize(rest)?;
            continue;
        }

        if param_str.starts_with('-') && param_str != "-" {
            return Err(SdiffError::UnrecognizedOption(param_str.to_string()));
        }

        operands.push(param);
    }

    if operands.len() != 2 {
        return Err(SdiffError::Usage(
            params.executable.to_string_lossy().into_owned(),
        ));
    }

    params.from = operands.remove(0);
    params.to = operands.remove(0);

    Ok(params)
}

pub fn compare_files(params: &Params) -> Outcome {
    if params.from == "-" && params.to == "-"
        || same_file::is_same_file(&params.from, &params.to).unwrap_or(false)
    {
        if params.suppress_common_lines {
            return Outcome::Identical;
        }
        let content = match read_file_contents(&params.from) {
            Ok(c) => c,
            Err(e) => {
                report_failure_to_read_input_file(&params.executable, &params.from, &e);
                return Outcome::Trouble;
            }
        };
        if !params.text && is_binary(&content) {
            return Outcome::Identical;
        }
        let mut stdout = stdout().lock();
        side_diff::diff(&content, &content, &mut stdout, params);
        let _ = stdout.flush();
        return Outcome::Identical;
    }

    let mut io_error = false;
    let from_content = match read_file_contents(&params.from) {
        Ok(c) => c,
        Err(e) => {
            report_failure_to_read_input_file(&params.executable, &params.from, &e);
            io_error = true;
            vec![]
        }
    };

    let to_content = match read_file_contents(&params.to) {
        Ok(c) => c,
        Err(e) => {
            report_failure_to_read_input_file(&params.executable, &params.to, &e);
            io_error = true;
            vec![]
        }
    };

    if io_error {
        return Outcome::Trouble;
    }

    if !params.text && (is_binary(&from_content) || is_binary(&to_content)) {
        if from_content == to_content {
            return Outcome::Identical;
        }
        println!(
            "Binary files {} and {} differ",
            params.from.to_string_lossy(),
            params.to.to_string_lossy()
        );
        return Outcome::Different;
    }

    let mut stdout = stdout().lock();
    let has_diff = side_diff::diff(&from_content, &to_content, &mut stdout, params);
    let _ = stdout.flush();

    if has_diff {
        Outcome::Different
    } else {
        Outcome::Identical
    }
}

pub fn main(opts: Peekable<ArgsOs>) -> ExitCode {
    let params = match parse_params(opts) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };

    compare_files(&params).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;
    use tempfile::NamedTempFile;

    fn parse(args: &[&str]) -> Result<Params, SdiffError> {
        let os_args: Vec<OsString> = args.iter().map(OsString::from).collect();
        parse_params(os_args.into_iter().peekable())
    }

    #[test]
    fn test_parse_defaults() {
        let p = parse(&["sdiff", "file1", "file2"]).unwrap();
        assert_eq!(p.from, OsString::from("file1"));
        assert_eq!(p.to, OsString::from("file2"));
        assert_eq!(p.width, 130);
        assert_eq!(p.tabsize, 8);
        assert!(!p.left_column);
        assert!(!p.suppress_common_lines);
        assert!(!p.expand_tabs);
        assert!(!p.text);
    }

    #[test]
    fn test_parse_all_flags() {
        let p = parse(&[
            "sdiff",
            "-s",
            "-l",
            "-t",
            "-a",
            "-w",
            "80",
            "--tabsize=4",
            "f1",
            "f2",
        ])
        .unwrap();
        assert!(p.suppress_common_lines);
        assert!(p.left_column);
        assert!(p.expand_tabs);
        assert!(p.text);
        assert_eq!(p.width, 80);
        assert_eq!(p.tabsize, 4);
    }

    #[test]
    fn test_parse_attached_width() {
        let p1 = parse(&["sdiff", "--width=100", "f1", "f2"]).unwrap();
        assert_eq!(p1.width, 100);

        let p2 = parse(&["sdiff", "-w95", "f1", "f2"]).unwrap();
        assert_eq!(p2.width, 95);
    }

    #[test]
    fn test_invalid_width_rejected() {
        assert_eq!(
            parse(&["sdiff", "-w", "6", "f1", "f2"]).unwrap_err(),
            SdiffError::InvalidWidth("6".to_string())
        );
        assert_eq!(
            parse(&["sdiff", "-w", "not_a_num", "f1", "f2"]).unwrap_err(),
            SdiffError::InvalidWidth("not_a_num".to_string())
        );
        assert_eq!(
            parse(&["sdiff", "-w"]).unwrap_err(),
            SdiffError::MissingArgument("-w".to_string())
        );
        assert_eq!(
            parse(&["sdiff", "--width"]).unwrap_err(),
            SdiffError::MissingArgument("--width".to_string())
        );
    }

    #[test]
    fn test_tabsize_parsing() {
        let p1 = parse(&["sdiff", "--tabsize=4", "f1", "f2"]).unwrap();
        assert_eq!(p1.tabsize, 4);

        let p2 = parse(&["sdiff", "--tabsize", "2", "f1", "f2"]).unwrap();
        assert_eq!(p2.tabsize, 2);

        assert_eq!(
            parse(&["sdiff", "--tabsize=0", "f1", "f2"]).unwrap_err(),
            SdiffError::InvalidTabSize("0".to_string())
        );
        assert_eq!(
            parse(&["sdiff", "--tabsize", "0", "f1", "f2"]).unwrap_err(),
            SdiffError::InvalidTabSize("0".to_string())
        );
        assert_eq!(
            parse(&["sdiff", "--tabsize=invalid", "f1", "f2"]).unwrap_err(),
            SdiffError::InvalidTabSize("invalid".to_string())
        );
        assert_eq!(
            parse(&["sdiff", "--tabsize"]).unwrap_err(),
            SdiffError::MissingArgument("--tabsize".to_string())
        );
    }

    #[test]
    fn test_missing_operands() {
        assert_eq!(
            parse(&["sdiff", "f1"]).unwrap_err(),
            SdiffError::Usage("sdiff".to_string())
        );
        assert_eq!(
            parse(&["sdiff"]).unwrap_err(),
            SdiffError::Usage("sdiff".to_string())
        );
    }

    #[test]
    fn test_extra_operands() {
        assert_eq!(
            parse(&["sdiff", "f1", "f2", "f3"]).unwrap_err(),
            SdiffError::Usage("sdiff".to_string())
        );
    }

    #[test]
    fn test_double_dash_operand_separator() {
        let p = parse(&["sdiff", "--", "-weird-file1", "-weird-file2"]).unwrap();
        assert_eq!(p.from, OsString::from("-weird-file1"));
        assert_eq!(p.to, OsString::from("-weird-file2"));
    }

    #[test]
    fn test_unrecognized_option() {
        let res = parse(&["sdiff", "-unknown", "f1", "f2"]);
        assert_eq!(
            res.unwrap_err(),
            SdiffError::UnrecognizedOption("-unknown".to_string())
        );
    }

    #[test]
    fn test_outcome_exit_codes() {
        assert_eq!(ExitCode::from(Outcome::Identical), ExitCode::SUCCESS);
        assert_eq!(ExitCode::from(Outcome::Different), ExitCode::from(1));
        assert_eq!(ExitCode::from(Outcome::Trouble), ExitCode::from(2));
    }

    #[test]
    fn test_compare_files_identical() {
        let f1 = NamedTempFile::new().unwrap();
        fs::write(f1.path(), b"hello\nworld\n").unwrap();

        let params = Params {
            executable: OsString::from("sdiff"),
            from: f1.path().as_os_str().to_os_string(),
            to: f1.path().as_os_str().to_os_string(),
            ..Default::default()
        };

        assert_eq!(compare_files(&params), Outcome::Identical);
    }

    #[test]
    fn test_compare_files_different() {
        let f1 = NamedTempFile::new().unwrap();
        let f2 = NamedTempFile::new().unwrap();
        fs::write(f1.path(), b"hello\nleft\n").unwrap();
        fs::write(f2.path(), b"hello\nright\n").unwrap();

        let params = Params {
            executable: OsString::from("sdiff"),
            from: f1.path().as_os_str().to_os_string(),
            to: f2.path().as_os_str().to_os_string(),
            ..Default::default()
        };

        assert_eq!(compare_files(&params), Outcome::Different);
    }

    #[test]
    fn test_compare_files_missing() {
        let params = Params {
            executable: OsString::from("sdiff"),
            from: OsString::from("/path/to/nonexistent_file_a"),
            to: OsString::from("/path/to/nonexistent_file_b"),
            ..Default::default()
        };

        assert_eq!(compare_files(&params), Outcome::Trouble);
    }

    #[test]
    fn test_compare_files_binary_different() {
        let f1 = NamedTempFile::new().unwrap();
        let f2 = NamedTempFile::new().unwrap();
        fs::write(f1.path(), b"\0binary1").unwrap();
        fs::write(f2.path(), b"\0binary2").unwrap();

        let params = Params {
            executable: OsString::from("sdiff"),
            from: f1.path().as_os_str().to_os_string(),
            to: f2.path().as_os_str().to_os_string(),
            ..Default::default()
        };

        assert_eq!(compare_files(&params), Outcome::Different);
    }
}
