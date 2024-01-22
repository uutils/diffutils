// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::params::*;
use std::env;

use std::fs;
use std::io::{self, Write};

mod params;
mod normal_diff;
mod unified_diff;
mod context_diff;
mod ed_diff;

fn main() -> Result<(), String> {
    let opts = env::args_os();
    let Params {
        from,
        to,
        context_count,
        format,
    } = parse_params(opts)?;
    // read files
    let from_content = match fs::read(&from) {
        Ok(from_content) => from_content,
        Err(e) => {
            return Err(format!("Failed to read from-file: {}", e));
        }
    };
    let to_content = match fs::read(&to) {
        Ok(to_content) => to_content,
        Err(e) => {
            return Err(format!("Failed to read from-file: {}", e));
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
        Format::Ed => ed_diff::diff(&from_content, &to_content)?,
    };
    io::stdout().write_all(&result).unwrap();
    Ok(())
}
