// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

pub mod context_diff;
pub mod ed_diff;
pub mod normal_diff;
pub mod params;
pub mod side_diff;
pub mod unified_diff;

use crate::params::{parse_params, Format};
use std::ffi::OsString;
use std::fs;
use std::io::{self, stdout, Read, Write};
// use std::process::{ExitCode, exit};
use uucore::error::{FromIo, UResult};
use uudiff::utils::{format_io_error, report_failure_to_read_input_file};

// Exit codes are documented at
// https://www.gnu.org/software/diffutils/manual/html_node/Invoking-diff.html.
//     An exit status of 0 means no differences were found,
//     1 means some differences were found,
//     and 2 means trouble.
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let args = args.peekable();
    let params = match parse_params(args) {
        Ok(p) => p,
        Err(error) => {
            eprintln!("{error}");
            uucore::error::set_exit_code(2);
            return Ok(());
        }
    };
    // if from and to are the same file, no need to perform any comparison
    let maybe_report_identical_files = || {
        if params.report_identical_files {
            println!(
                "Files {} and {} are identical",
                params.from.to_string_lossy(),
                params.to.to_string_lossy(),
            );
        }
    };
    if params.from == "-" && params.to == "-"
        || same_file::is_same_file(&params.from, &params.to).unwrap_or(false)
    {
        maybe_report_identical_files();
        // ExitCode::SUCCESS;
        uucore::error::set_exit_code(0);
        return Ok(());
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
    let mut io_error = false;
    let from_content = match read_file_contents(&params.from) {
        Ok(from_content) => from_content,
        Err(e) => {
            report_failure_to_read_input_file(&params.executable, &params.from, &e);
            io_error = true;
            vec![]
        }
    };
    let to_content = match read_file_contents(&params.to) {
        Ok(to_content) => to_content,
        Err(e) => {
            report_failure_to_read_input_file(&params.executable, &params.to, &e);
            io_error = true;
            vec![]
        }
    };
    if io_error {
        uucore::error::set_exit_code(2);
        return Ok(());
    }

    // run diff
    let result: Vec<u8> = match params.format {
        Format::Normal => normal_diff::diff(&from_content, &to_content, &params),
        Format::Unified => unified_diff::diff(&from_content, &to_content, &params),
        Format::Context => context_diff::diff(&from_content, &to_content, &params),
        Format::Ed => ed_diff::diff(&from_content, &to_content, &params).unwrap_or_else(|error| {
            eprintln!("{error}");
            uucore::error::set_exit_code(2);
            std::process::exit(2);
        }),
        Format::SideBySide => {
            let mut output = stdout().lock();
            side_diff::diff(&from_content, &to_content, &mut output, &params)
        }
    };
    if params.brief && !result.is_empty() {
        println!(
            "Files {} and {} differ",
            params.from.to_string_lossy(),
            params.to.to_string_lossy()
        );
    } else {
        let result = io::stdout().write_all(&result);
        match result {
            // This code is taken from coreutils.
            // <https://github.com/uutils/coreutils/blob/main/src/uu/seq/src/seq.rs>
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => {
                // GNU seq prints the Broken pipe message but still exits with status 0
                // unless SIGPIPE was explicitly ignored, in which case it should fail.
                let err = err.map_err_context(|| "write error".into());
                uucore::show_error!("{err}");
                #[cfg(unix)]
                if uucore::signals::sigpipe_was_ignored() {
                    uucore::error::set_exit_code(1);
                }
            }
            Err(error) => {
                eprintln!("{}", format_io_error(&error));
                uucore::error::set_exit_code(1);
                return Ok(());
            }
        }
    }
    if result.is_empty() {
        maybe_report_identical_files();
        // ExitCode::SUCCESS;
        uucore::error::set_exit_code(0);
    } else {
        uucore::error::set_exit_code(1);
    }

    Ok(())
}
