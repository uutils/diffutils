// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::{
    ffi::{OsStr, OsString},
    iter::Peekable,
    path::{Path, PathBuf},
    process::ExitCode,
};

/// # Panics
/// Panics if the binary path cannot be determined
fn binary_path<I: Iterator<Item = OsString>>(args: &mut Peekable<I>) -> PathBuf {
    match args.peek() {
        Some(ref s) if !s.is_empty() => PathBuf::from(s),
        _ => std::env::current_exe().unwrap(),
    }
}

/// #Panics
/// Panics if path has no UTF-8 valid name
fn name(binary_path: &Path) -> &OsStr {
    binary_path.file_stem().unwrap()
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage(name: &str) {
    println!("{name} {VERSION} (multi-call binary)\n");
    println!("Usage: {name} [function [arguments...]]\n");
    println!("Currently defined functions:\n");
    println!("    cmp, diff\n");
}

fn second_arg_error(name: &OsStr) -> ! {
    eprintln!("Expected utility name as second argument, got nothing.");
    usage(&name.to_string_lossy());
    std::process::exit(0);
}

fn main() -> ExitCode {
    let mut args = uucore::args_os().peekable();

    let exe_path = binary_path(&mut args);
    let exe_name = name(&exe_path);

    let util_name = if exe_name == "diffutils" {
        // Discard the item we peeked.
        let _ = args.next();

        args.peek()
            .cloned()
            .unwrap_or_else(|| second_arg_error(exe_name))
    } else {
        OsString::from(exe_name)
    };

    let code = match util_name.to_str() {
        Some("cmp") => cmp::uumain(args),
        Some("diff") => diff::uumain(args),
        Some(name) => {
            eprintln!("{name}: utility not supported");
            // ExitCode::from(2)
            2
        }
        None => second_arg_error(exe_name),
    };

    ExitCode::from(code as u8)
}
