// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// Re-export the public functions/types you need
// pub use context_diff::diff as context_diff;
// pub use ed_diff::diff as ed_diff;
// pub use normal_diff::diff as normal_diff;
// pub use side_diff::diff as side_by_side_diff;
// pub use unified_diff::diff as unified_diff;

pub mod context_diff;
pub mod ed_diff;
pub mod normal_diff;
pub mod params_diff;
// not used anymore, only for bench
pub mod params_old;
pub mod side_diff;
pub mod unified_diff;

use crate::params_diff::{FormatOutput, Params};
use clap::Command;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write, stdout};
use uudiff::common_errors::UtilsError;
use uudiff::error::{FromIo, UIoError, UResult};
use uudiff::utils::CompareOk;
use uudiff::{translate, utils};

/// Entry into diff.
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let args_checked = clap_preparation(args);
    let matches =
        uudiff::clap_localization::handle_clap_result_with_exit_code(uu_app(), args_checked, 2)?;

    let params: Params = matches.try_into()?;

    let res = diff_compare(&params)?;
    match res {
        CompareOk::Equal => uucore::error::set_exit_code(0),
        CompareOk::Different => uucore::error::set_exit_code(1),
    }

    Ok(())
}

pub fn clap_preparation(args: impl uucore::Args) -> Vec<OsString> {
    // handle constellations, clap can't do
    // so clap is limited to -c=num, while GNU allows -c42 and -42c (and 4c2)
    let mut args_checked = Vec::new();
    for mut arg_os in args {
        if arg_os.len() > 2 {
            let arg = arg_os.to_string_lossy();
            if arg.as_bytes()[0] == b'-' {
                // short options with num or multiple short options
                let mut opt = '-';
                let mut num = String::new();
                let mut ok = false;
                // let c = arg.as_bytes()[1] as char;
                for c in arg.chars().skip(1) {
                    if c.is_ascii_digit() {
                        num.push(c);
                    } else if c.is_ascii_lowercase() {
                        // possibly multi-single-options, e.g. -sc4 is valid
                        if c == 'c' || c == 'u' {
                            if opt == '-' {
                                opt = c;
                                ok = true;
                            } else {
                                // multiple chars, reject
                                ok = false;
                                break;
                            }
                        }
                    } else {
                        // unknown char, reject
                        ok = false;
                        break;
                    }
                }
                if ok {
                    // create c=42 structure
                    let mut s = String::from("-");
                    s.push(opt);
                    s.push('=');
                    s.push_str(&num);
                    arg_os = s.into();
                }
            }
        }
        // dbg!(&arg_os);
        args_checked.push(arg_os);
    }

    args_checked
}

pub fn diff_compare(params: &Params) -> UResult<CompareOk> {
    let maybe_report_identical_files = || {
        if params.report_identical_files {
            let msg = translate!("diff-info-files-are-identical", 
            "file_1" => params.from.to_string_lossy(), 
            "file_2" => params.to.to_string_lossy());
            println!("{msg}");
        }
    };

    // if from and to are the same file, no need to perform any comparison
    if utils::is_same_file(&params.from, &params.to) {
        maybe_report_identical_files();
        return Ok(CompareOk::Equal);
    }

    // read files
    fn read_file_contents(filepath: &OsString) -> io::Result<Vec<u8>> {
        if filepath == "-" {
            let mut content = Vec::new();
            io::stdin().read_to_end(&mut content).and(Ok(content))
        } else {
            fs::read(filepath)
        }
    }

    // UIoError has no code https://github.com/uutils/coreutils/issues/11453
    let r_from_content = read_file_contents(&params.from);
    let r_to_content = read_file_contents(&params.to);

    // Diff returns both errors
    let from_content = match r_from_content {
        Ok(c) => c,
        Err(e1) => match r_to_content {
            Ok(_) => {
                let io = e1.map_err_context(|| params.from_as_string_lossy());
                return Err(UtilsError::Io(io).into());
            }
            Err(e2) => {
                let io1 = e1.map_err_context(|| params.from_as_string_lossy());
                let io2 = e2.map_err_context(|| params.to_as_string_lossy());
                return Err(UtilsError::IoDouble(io1, io2).into());
            }
        },
    };
    let to_content = match r_to_content {
        Ok(c) => c,
        Err(e2) => {
            let io = e2.map_err_context(|| params.to_as_string_lossy());
            return Err(UtilsError::Io(io).into());
        }
    };

    // run diff
    let result: Vec<u8> = match params.format_out {
        FormatOutput::Normal => normal_diff::diff(&from_content, &to_content, params),
        FormatOutput::Unified => unified_diff::diff(&from_content, &to_content, params),
        FormatOutput::Context => context_diff::diff(&from_content, &to_content, params),
        FormatOutput::Ed => ed_diff::diff(&from_content, &to_content, params)?,
        FormatOutput::SideBySide => {
            let mut output = stdout().lock();
            side_diff::diff(&from_content, &to_content, &mut output, params)
        }
    };

    #[allow(clippy::redundant_else)]
    if params.brief && !result.is_empty() {
        let msg = translate!("diff-info-files-are-different", 
                "file_1" => params.from.to_string_lossy(), 
                "file_2" => params.to.to_string_lossy());
        println!("{msg}");
        return Ok(CompareOk::Different);
    } else {
        let result = io::stdout().write_all(&result);
        match result {
            // This code is adapted from coreutils.
            // <https://github.com/uutils/coreutils/blob/main/src/uu/seq/src/seq.rs>
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => {
                // GNU seq prints the Broken pipe message but still exits with status 0
                // unless SIGPIPE was explicitly ignored, in which case it should fail.
                let err = err.map_err_context(|| "write error".into());
                uucore::show_error!("{err}");
                #[cfg(unix)]
                if uucore::signals::sigpipe_was_ignored() {
                    uucore::error::set_exit_code(0);
                }
            }
            Err(error) => {
                let io = UIoError::from(error);
                return Err(UtilsError::Io(io.into()).into());
            }
        }
    }

    if result.is_empty() {
        maybe_report_identical_files();
        Ok(CompareOk::Equal)
    } else {
        Ok(CompareOk::Different)
    }
}

/// Contains all diff errors and their text messages.
///
/// All errors can be output easily using the normal Display functionality.
/// To format the error message for the typical diffutils output, use [format_error_text].
#[derive(Debug, PartialEq, Eq)]
pub enum DiffError {
    MissingNL,
}

impl std::error::Error for DiffError {}

impl uudiff::error::UError for DiffError {
    fn code(&self) -> i32 {
        2
    }
}

impl std::fmt::Display for DiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::MissingNL => translate!("diff-error-missing-newline"),
        };

        write!(f, "{msg}")
    }
}

// Required for build.rs
pub fn uu_app() -> Command {
    crate::params_diff::uu_app()
}
