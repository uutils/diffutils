// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::params::{parse_params, Format};
use crate::utils::report_failure_to_read_input_file;
use crate::{context_diff, ed_diff, normal_diff, unified_diff};
use std::env::ArgsOs;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::iter::Peekable;
use std::process::{exit, ExitCode};

// Exit codes are documented at
// https://www.gnu.org/software/diffutils/manual/html_node/Invoking-diff.html.
//     An exit status of 0 means no differences were found,
//     1 means some differences were found,
//     and 2 means trouble.
pub(crate) fn main(opts: Peekable<ArgsOs>) -> ExitCode {
    let params = parse_params(opts).unwrap_or_else(|error| {
        eprintln!("{error}");
        exit(2);
    });
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
        return ExitCode::SUCCESS;
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
        return ExitCode::from(2);
    }

    // run diff
    let result: Vec<u8> = match params.format {
        Format::Normal => normal_diff::diff(&from_content, &to_content, &params),
        Format::Unified => unified_diff::diff(&from_content, &to_content, &params),
        Format::Context => context_diff::diff(&from_content, &to_content, &params),
        Format::Ed => ed_diff::diff(&from_content, &to_content, &params).unwrap_or_else(|error| {
            eprintln!("{error}");
            exit(2);
        }),
    };
    if params.brief && !result.is_empty() {
        println!(
            "Files {} and {} differ",
            params.from.to_string_lossy(),
            params.to.to_string_lossy()
        );
    } else {
        io::stdout().write_all(&result).unwrap();
    }
    if result.is_empty() {
        maybe_report_identical_files();
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
