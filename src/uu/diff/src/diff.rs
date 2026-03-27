// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
pub mod context_diff;
pub mod ed_diff;
pub mod macros;
pub mod normal_diff;
pub mod params;
pub mod side_diff;
pub mod unified_diff;

// Re-export the public functions/types you need
// TODO remove pub?
pub use context_diff::diff as context_diff;
pub use ed_diff::diff as ed_diff;
pub use normal_diff::diff as normal_diff;
pub use side_diff::diff as side_by_side_diff;
pub use unified_diff::diff as unified_diff;

use crate::params::{Format, parse_params};
use clap::Command;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write, stdout};
use std::process::exit;
use uucore::error::set_exit_code;
use uudiff::error::UResult;
use uudiff::utils::report_failure_to_read_input_file;

/// Entry into diff.
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let opts = args.peekable();
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
        set_exit_code(0);
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
        set_exit_code(2);
        return Ok(());
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
        io::stdout().write_all(&result).unwrap();
    }
    if result.is_empty() {
        maybe_report_identical_files();
        set_exit_code(0);
    } else {
        set_exit_code(1);
    }
    Ok(())
}

// Required for build.rs
pub fn uu_app() -> Command {
    // dummy
    Command::new(uucore::util_name())
}
