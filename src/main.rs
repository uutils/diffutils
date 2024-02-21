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
    } = parse_params(opts).unwrap_or_else(|error| {
        eprintln!("{error}");
        exit(2);
    });
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
        Format::Normal => normal_diff::diff(&from_content, &to_content),
        Format::Unified => unified_diff::diff(
            &from_content,
            &from.to_string_lossy(),
            &to_content,
            &to.to_string_lossy(),
            context_count,
        ),
        Format::Context => context_diff::diff(
            &from_content,
            &from.to_string_lossy(),
            &to_content,
            &to.to_string_lossy(),
            context_count,
        ),
        Format::Ed => ed_diff::diff(&from_content, &to_content).unwrap_or_else(|error| {
            eprintln!("{error}");
            exit(2);
        }),
    };
    io::stdout().write_all(&result).unwrap();
    if result.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
