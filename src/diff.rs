// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::color::{colorize, ColorWhen, Palette, PaletteError};
use crate::params::{parse_params, Format, MissingFile, Params};
use crate::utils::report_failure_to_read_input_file;
use crate::{context_diff, dir_diff, ed_diff, normal_diff, side_diff, unified_diff};
use std::env::ArgsOs;
use std::ffi::OsString;
use std::fs;
use std::io::{self, stdout, IsTerminal, Read, Write};
use std::iter::Peekable;
use std::path::Path;
use std::process::ExitCode;

/// The result of a comparison, from best to worst, so that the outcome of
/// several comparisons is their maximum.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Outcome {
    Identical,
    Different,
    Trouble,
}

// Exit codes are documented at
// https://www.gnu.org/software/diffutils/manual/html_node/Invoking-diff.html.
//     An exit status of 0 means no differences were found,
//     1 means some differences were found,
//     and 2 means trouble.
impl From<Outcome> for ExitCode {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Identical => Self::SUCCESS,
            Outcome::Different => Self::from(1),
            Outcome::Trouble => Self::from(2),
        }
    }
}

pub fn main(opts: Peekable<ArgsOs>) -> ExitCode {
    let (mut params, option_args) = match parse_params(opts) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    // diff - FILE with a directory on stdin => diff DIRECTORY/FILE FILE
    if params.from == "-" && params.to != "-" {
        if let Some(from) = stdin_directory_entry(&params.to) {
            params.from = from;
        }
    }
    let both_directories = Path::new(&params.from).is_dir() && Path::new(&params.to).is_dir();
    // Directories, and files whose symbolic links are not followed, are
    // compared by type before their contents.
    let compare_types =
        params.from != "-" && params.to != "-" && (both_directories || params.no_dereference);
    let outcome = if compare_types {
        dir_diff::compare(&params, &option_args)
    } else {
        compare_files(&params, None)
    };
    outcome.into()
}

/// Compares the files `params.from` and `params.to`, printing their
/// differences. When the files differ, `header` is printed first unless only
/// a brief report is requested.
pub fn compare_files(params: &Params, header: Option<&[u8]>) -> Outcome {
    let maybe_report_identical_files = || {
        if params.report_identical_files {
            println!(
                "Files {} and {} are identical",
                params.from.to_string_lossy(),
                params.to.to_string_lossy(),
            );
        }
    };
    // if from and to are the same file, no need to perform any comparison
    if params.from == "-" && params.to == "-"
        || same_file::is_same_file(&params.from, &params.to).unwrap_or(false)
    {
        maybe_report_identical_files();
        return Outcome::Identical;
    }

    // read files, along with how many leading bytes decide if they are binary
    fn read_file_contents(filepath: &OsString) -> io::Result<(Vec<u8>, usize)> {
        let mut content = Vec::new();
        if filepath == "-" {
            io::stdin().read_to_end(&mut content)?;
            Ok((content, stdin_binary_probe_len()))
        } else {
            let mut file = fs::File::open(filepath)?;
            file.read_to_end(&mut content)?;
            Ok((content, binary_probe_len(&file)))
        }
    }
    let mut io_error = false;
    let mut from_missing = false;
    let mut to_missing = false;
    let (from_content, from_probe_len) = match read_file_contents(&params.from) {
        Ok(from_content) => from_content,
        Err(e)
            if e.kind() == io::ErrorKind::NotFound && params.missing_file != MissingFile::Error =>
        {
            from_missing = true;
            (vec![], 0)
        }
        Err(e) => {
            report_failure_to_read_input_file(&params.executable, &params.from, &e);
            io_error = true;
            (vec![], 0)
        }
    };
    let (to_content, to_probe_len) = match read_file_contents(&params.to) {
        Ok(to_content) => to_content,
        Err(e)
            if e.kind() == io::ErrorKind::NotFound && params.missing_file == MissingFile::Empty =>
        {
            to_missing = true;
            (vec![], 0)
        }
        Err(e) => {
            report_failure_to_read_input_file(&params.executable, &params.to, &e);
            io_error = true;
            (vec![], 0)
        }
    };
    if io_error {
        return Outcome::Trouble;
    }

    if !params.text
        && (is_binary(&from_content, from_probe_len) || is_binary(&to_content, to_probe_len))
    {
        if from_content == to_content {
            maybe_report_identical_files();
            return Outcome::Identical;
        }
        print_files_differ(
            if params.brief {
                "Files"
            } else {
                "Binary files"
            },
            params,
        );
        return Outcome::Different;
    }

    let params = &Params {
        from_missing,
        to_missing,
        ..params.clone()
    };

    // run diff
    let result: Vec<u8> = match params.format {
        Format::Normal => normal_diff::diff(&from_content, &to_content, params),
        Format::Unified => unified_diff::diff(&from_content, &to_content, params),
        Format::Context => context_diff::diff(&from_content, &to_content, params),
        Format::Ed => match ed_diff::diff(&from_content, &to_content, params) {
            Ok(result) => result,
            Err(error) => {
                eprintln!("{error}");
                return Outcome::Trouble;
            }
        },
        // Side by side output is streamed, unless a header may have to come
        // first.
        Format::SideBySide => match header {
            None => side_diff::diff(&from_content, &to_content, &mut stdout().lock(), params),
            Some(header) => {
                let mut output = Vec::new();
                let result = side_diff::diff(&from_content, &to_content, &mut output, params);
                if !result.is_empty() {
                    output.splice(0..0, header.iter().copied());
                }
                if stdout().write_all(&output).is_err() {
                    return Outcome::Trouble;
                }
                result
            }
        },
    };
    if params.brief && !result.is_empty() {
        print_files_differ("Files", params);
    } else if params.format != Format::SideBySide {
        let header = if result.is_empty() {
            &[][..]
        } else {
            header.unwrap_or_default()
        };
        let colored =
            output_palette(params).map(|palette| colorize(&result, params.format, &palette));
        let mut stdout = stdout().lock();
        let written = stdout
            .write_all(header)
            .and_then(|()| stdout.write_all(colored.as_deref().unwrap_or(&result)))
            .and_then(|()| stdout.flush());
        if let Err(error) = written {
            // A reader that went away, as with `| head`, is not worth a message.
            if error.kind() != io::ErrorKind::BrokenPipe {
                report_failure_to_read_input_file(
                    &params.executable,
                    &OsString::from("standard output"),
                    &error,
                );
            }
            return Outcome::Trouble;
        }
    }
    if result.is_empty() {
        maybe_report_identical_files();
        Outcome::Identical
    } else {
        Outcome::Different
    }
}

/// Returns the palette to colorize the output with, or `None` when the
/// output is not to be colorized. An invalid `--palette` is reported and
/// disables colors.
fn output_palette(params: &Params) -> Option<Palette> {
    let enabled = match params.color {
        ColorWhen::Never => false,
        ColorWhen::Always => true,
        ColorWhen::Auto => {
            (params.presume_output_tty || stdout().is_terminal())
                && std::env::var_os("TERM").is_none_or(|term| term != "dumb")
        }
    };
    if !enabled {
        return None;
    }
    let Some(spec) = &params.palette else {
        return Some(Palette::default());
    };
    let executable = params.executable.to_string_lossy();
    match Palette::parse(spec) {
        Ok(palette) => Some(palette),
        Err(error) => {
            if let PaletteError::UnrecognizedPrefix(prefix) = error {
                eprintln!("{executable}: unrecognized prefix: {prefix}");
            }
            eprintln!("{executable}: unparsable value for --palette");
            None
        }
    }
}

/// Number of leading bytes inspected when deciding whether an input is binary,
/// where the file system does not tell its preferred I/O size.
const DEFAULT_BINARY_PROBE_LEN: usize = 16 * 1024;

/// An input is treated as binary when a NUL byte occurs in its first
/// `probe_len` bytes.
fn is_binary(content: &[u8], probe_len: usize) -> bool {
    content[..content.len().min(probe_len)].contains(&0)
}

/// Like GNU diff, only the first buffer read from a file decides whether it
/// is binary, and that buffer is as large as the preferred I/O size of the
/// file (`st_blksize`).
#[cfg(unix)]
fn binary_probe_len(file: &fs::File) -> usize {
    use std::os::unix::fs::MetadataExt;
    file.metadata()
        .ok()
        .and_then(|metadata| usize::try_from(metadata.blksize()).ok())
        .filter(|&len| len > 0)
        .unwrap_or(DEFAULT_BINARY_PROBE_LEN)
}

#[cfg(not(unix))]
fn binary_probe_len(_file: &fs::File) -> usize {
    DEFAULT_BINARY_PROBE_LEN
}

#[cfg(unix)]
fn stdin_binary_probe_len() -> usize {
    use std::os::fd::AsFd;
    io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map_or(DEFAULT_BINARY_PROBE_LEN, |fd| {
            binary_probe_len(&fs::File::from(fd))
        })
}

#[cfg(not(unix))]
fn stdin_binary_probe_len() -> usize {
    DEFAULT_BINARY_PROBE_LEN
}

/// Print the one-line summary used instead of the differences, `kind` naming
/// what was compared.
fn print_files_differ(kind: &str, params: &Params) {
    println!(
        "{kind} {} and {} differ",
        params.from.to_string_lossy(),
        params.to.to_string_lossy()
    );
}

/// When standard input is a directory, returns the path of the entry in it
/// that has the same name as `other`, provided `other` is not a directory.
/// Entries are reached through `/dev/fd/0`, which only names the directory
/// itself where it links to the open file, as with Linux procfs.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn stdin_directory_entry(other: &OsString) -> Option<OsString> {
    use std::os::fd::AsFd;

    let stdin_is_dir = io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .ok()
        .and_then(|fd| fs::File::from(fd).metadata().ok())
        .is_some_and(|metadata| metadata.is_dir());
    let other = Path::new(other);
    if !stdin_is_dir || other.is_dir() {
        return None;
    }
    Some(
        Path::new("/dev/fd/0")
            .join(other.file_name()?)
            .into_os_string(),
    )
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn stdin_directory_entry(_other: &OsString) -> Option<OsString> {
    None
}
