// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::{
    env::ArgsOs,
    ffi::OsString,
    iter::Peekable,
    path::{Path, PathBuf},
    process::ExitCode,
};

mod context_diff;
mod diff;
mod ed_diff;
mod macros;
mod normal_diff;
mod params;
mod unified_diff;
mod utils;

/// # Panics
/// Panics if the binary path cannot be determined
fn binary_path(args: &mut Peekable<ArgsOs>) -> PathBuf {
    match args.peek() {
        Some(ref s) if !s.is_empty() => PathBuf::from(s),
        _ => std::env::current_exe().unwrap(),
    }
}

fn name(binary_path: &Path) -> Option<&str> {
    binary_path.file_stem()?.to_str()
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage(name: &str) {
    println!("{name} {VERSION} (multi-call binary)\n");
    println!("Usage: {name} [function [arguments...]]\n");
    println!("Currently defined functions:\n");
    println!("    diff\n");
}

fn second_arg_error(name: &str) -> ! {
    println!("Expected utility name as second argument, got nothing.");
    usage(name);
    std::process::exit(0);
}

fn main() -> ExitCode {
    let mut args = std::env::args_os().peekable();

    let exe_path = binary_path(&mut args);
    let exe_name = name(&exe_path).unwrap_or_else(|| {
        usage("<unknown binary>");
        std::process::exit(1);
    });

    let util_name = if exe_name == "diffutils" {
        // Discard the item we peeked.
        let _ = args.next();

        args.peek()
            .cloned()
            .unwrap_or_else(|| second_arg_error(exe_name))
    } else {
        OsString::from(exe_name)
    };

    match util_name.to_str() {
        Some("diff") => diff::main(args),
        Some(name) => {
            usage(&format!("{}: utility not supported", name));
            ExitCode::from(1)
        }
        None => second_arg_error(exe_name),
    }
}
