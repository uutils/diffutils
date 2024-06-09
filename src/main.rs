// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::params::{parse_params, Format};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::process::{exit, ExitCode};

mod context_diff;
mod ed_diff;
mod macros;
mod normal_diff;
mod params;
mod unified_diff;
mod utils;

// Exit codes are documented at
// https://www.gnu.org/software/diffutils/manual/html_node/Invoking-diff.html.
//     An exit status of 0 means no differences were found,
//     1 means some differences were found,
//     and 2 means trouble.
fn main() -> ExitCode {
    let opts = env::args_os();
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
    fn read_file_contents(filepath: &OsString, stdin_path: &OsString) -> io::Result<Vec<u8>> {
        if filepath.to_string_lossy().starts_with('-') && !stdin_path.is_empty() {
            fs::read(stdin_path)
        } else if filepath == "-" {
            let mut content = Vec::new();
            io::stdin().read_to_end(&mut content).and(Ok(content))
        } else {
            fs::read(filepath)
        }
    }
    let from_content = match read_file_contents(&params.from, &params.stdin_path) {
        Ok(from_content) => from_content,
        Err(e) => {
            eprintln!("Failed to read from-file: {e}");
            return ExitCode::from(2);
        }
    };
    let to_content = match read_file_contents(&params.to, &params.stdin_path) {
        Ok(to_content) => to_content,
        Err(e) => {
            eprintln!("Failed to read to-file: {e}");
            return ExitCode::from(2);
        }
    };
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
