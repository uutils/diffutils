// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::params::{parse_params, Format, Params};
use std::env;

use std::fs;
use std::io::{self, Write};
use std::process::{exit, ExitCode};

mod context_diff;
mod ed_diff;
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
    let Params {
        from,
        to,
        context_count,
        format,
        report_identical_files,
        brief,
        expand_tabs,
        tabsize,
    } = parse_params(opts).unwrap_or_else(|error| {
        eprintln!("{error}");
        exit(2);
    });
    // if from and to are the same file, no need to perform any comparison
    let maybe_report_identical_files = || {
        if report_identical_files {
            println!(
                "Files {} and {} are identical",
                from.to_string_lossy(),
                to.to_string_lossy(),
            )
        }
    };
    if same_file::is_same_file(&from, &to).unwrap_or(false) {
        maybe_report_identical_files();
        return ExitCode::SUCCESS;
    }
    // read files
    let from_content = match fs::read(&from) {
        Ok(from_content) => from_content,
        Err(e) => {
            eprintln!("Failed to read from-file: {e}");
            return ExitCode::from(2);
        }
    };
    let to_content = match fs::read(&to) {
        Ok(to_content) => to_content,
        Err(e) => {
            eprintln!("Failed to read to-file: {e}");
            return ExitCode::from(2);
        }
    };
    // run diff
    let result: Vec<u8> = match format {
        Format::Normal => {
            normal_diff::diff(&from_content, &to_content, brief, expand_tabs, tabsize)
        }
        Format::Unified => unified_diff::diff(
            &from_content,
            &from.to_string_lossy(),
            &to_content,
            &to.to_string_lossy(),
            context_count,
            brief,
            expand_tabs,
            tabsize,
        ),
        Format::Context => context_diff::diff(
            &from_content,
            &from.to_string_lossy(),
            &to_content,
            &to.to_string_lossy(),
            context_count,
            brief,
            expand_tabs,
            tabsize,
        ),
        Format::Ed => ed_diff::diff(&from_content, &to_content, brief, expand_tabs, tabsize)
            .unwrap_or_else(|error| {
                eprintln!("{error}");
                exit(2);
            }),
    };
    if brief && !result.is_empty() {
        println!(
            "Files {} and {} differ",
            from.to_string_lossy(),
            to.to_string_lossy()
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
