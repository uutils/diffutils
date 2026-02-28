// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::utils::format_failure_to_read_input_file;
use std::env::{self, ArgsOs};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::iter::Peekable;
use std::process::ExitCode;
use std::{cmp, fs, io};

#[cfg(not(target_os = "windows"))]
use std::os::fd::{AsRawFd, FromRawFd};

#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::MetadataExt;

#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Params {
    executable: OsString,
    from: OsString,
    to: OsString,
    print_bytes: bool,
    skip_a: Option<usize>,
    skip_b: Option<usize>,
    max_bytes: Option<usize>,
    verbose: bool,
    quiet: bool,
}

#[inline]
fn usage_string(executable: &str) -> String {
    format!("Usage: {executable} <from> <to>")
}

#[cfg(not(target_os = "windows"))]
fn is_stdout_dev_null() -> bool {
    let Ok(dev_null) = fs::metadata("/dev/null") else {
        return false;
    };

    let stdout_fd = io::stdout().lock().as_raw_fd();

    // SAFETY: we have exclusive access to stdout right now.
    let stdout_file = unsafe { fs::File::from_raw_fd(stdout_fd) };
    let Ok(stdout) = stdout_file.metadata() else {
        return false;
    };

    let is_dev_null = stdout.dev() == dev_null.dev() && stdout.ino() == dev_null.ino();

    // Don't let File close the fd. It's unfortunate that File doesn't have a leak_fd().
    std::mem::forget(stdout_file);

    is_dev_null
}

pub fn parse_params<I: Iterator<Item = OsString>>(mut opts: Peekable<I>) -> Result<Params, String> {
    let Some(executable) = opts.next() else {
        return Err("Usage: <exe> <from> <to>".to_string());
    };
    let executable_str = executable.to_string_lossy().to_string();

    let parse_skip = |param: &str, skip_desc: &str| -> Result<usize, String> {
        let suffix_start = param
            .find(|b: char| !b.is_ascii_digit())
            .unwrap_or(param.len());
        let mut num = match param[..suffix_start].parse::<usize>() {
            Ok(num) => num,
            Err(e) if *e.kind() == std::num::IntErrorKind::PosOverflow => usize::MAX,
            Err(_) => {
                return Err(format!(
                    "{executable_str}: invalid --ignore-initial value '{skip_desc}'"
                ))
            }
        };

        if suffix_start != param.len() {
            // Note that GNU cmp advertises supporting up to Y, but fails if you try
            // to actually use anything beyond E.
            let multiplier: usize = match &param[suffix_start..] {
                "kB" => 1_000,
                "K" => 1_024,
                "MB" => 1_000_000,
                "M" => 1_048_576,
                "GB" => 1_000_000_000,
                "G" => 1_073_741_824,
                // This only generates a warning when compiling for target_pointer_width < 64
                #[allow(unused_variables)]
                suffix @ ("TB" | "T" | "PB" | "P" | "EB" | "E") => {
                    #[cfg(target_pointer_width = "64")]
                    match suffix {
                        "TB" => 1_000_000_000_000,
                        "T" => 1_099_511_627_776,
                        "PB" => 1_000_000_000_000_000,
                        "P" => 1_125_899_906_842_624,
                        "EB" => 1_000_000_000_000_000_000,
                        "E" => 1_152_921_504_606_846_976,
                        _ => unreachable!(),
                    }
                    #[cfg(not(target_pointer_width = "64"))]
                    usize::MAX
                }
                "ZB" => usize::MAX, // 1_000_000_000_000_000_000_000,
                "Z" => usize::MAX,  // 1_180_591_620_717_411_303_424,
                "YB" => usize::MAX, // 1_000_000_000_000_000_000_000_000,
                "Y" => usize::MAX,  // 1_208_925_819_614_629_174_706_176,
                _ => {
                    return Err(format!(
                        "{executable_str}: invalid --ignore-initial value '{skip_desc}'"
                    ));
                }
            };

            num = match num.overflowing_mul(multiplier) {
                (n, false) => n,
                _ => usize::MAX,
            }
        }

        Ok(num)
    };

    let mut params = Params {
        executable,
        ..Default::default()
    };
    let mut from = None;
    let mut to = None;
    let mut skip_pos1 = None;
    let mut skip_pos2 = None;
    while let Some(param) = opts.next() {
        if param == "--" {
            break;
        }
        if param == "-" {
            if from.is_none() {
                from = Some(param);
            } else if to.is_none() {
                to = Some(param);
            } else {
                return Err(usage_string(&executable_str));
            }
            continue;
        }
        if param == "-b" || param == "--print-bytes" {
            params.print_bytes = true;
            continue;
        }
        if param == "-l" || param == "--verbose" {
            params.verbose = true;
            continue;
        }
        if param == "-lb" || param == "-bl" {
            params.print_bytes = true;
            params.verbose = true;
            continue;
        }

        let param_str = param.to_string_lossy().to_string();
        if param == "-n" || param_str.starts_with("--bytes=") {
            let max_bytes = if param == "-n" {
                opts.next()
                    .ok_or_else(|| usage_string(&executable_str))?
                    .to_string_lossy()
                    .to_string()
            } else {
                let (_, arg) = param_str.split_once('=').unwrap();
                arg.to_string()
            };
            let max_bytes = match max_bytes.parse::<usize>() {
                Ok(num) => num,
                Err(e) if *e.kind() == std::num::IntErrorKind::PosOverflow => usize::MAX,
                Err(_) => {
                    return Err(format!(
                        "{executable_str}: invalid --bytes value '{max_bytes}'"
                    ))
                }
            };
            params.max_bytes = Some(max_bytes);
            continue;
        }
        if param == "-i" || param_str.starts_with("--ignore-initial=") {
            let skip_desc = if param == "-i" {
                opts.next()
                    .ok_or_else(|| usage_string(&executable_str))?
                    .to_string_lossy()
                    .to_string()
            } else {
                let (_, arg) = param_str.split_once('=').unwrap();
                arg.to_string()
            };
            let (skip_a, skip_b) = if let Some((skip_a, skip_b)) = skip_desc.split_once(':') {
                (
                    parse_skip(skip_a, &skip_desc)?,
                    parse_skip(skip_b, &skip_desc)?,
                )
            } else {
                let skip = parse_skip(&skip_desc, &skip_desc)?;
                (skip, skip)
            };
            params.skip_a = Some(skip_a);
            params.skip_b = Some(skip_b);
            continue;
        }
        if param == "-s" || param == "--quiet" || param == "--silent" {
            params.quiet = true;
            continue;
        }
        if param == "--help" {
            println!("{}", usage_string(&executable_str));
            std::process::exit(0);
        }
        if param_str.starts_with('-') {
            return Err(format!("unrecognized option: {param:?}"));
        }
        if from.is_none() {
            from = Some(param);
        } else if to.is_none() {
            to = Some(param);
        } else if skip_pos1.is_none() {
            skip_pos1 = Some(parse_skip(&param_str, &param_str)?);
        } else if skip_pos2.is_none() {
            skip_pos2 = Some(parse_skip(&param_str, &param_str)?);
        } else {
            return Err(usage_string(&executable_str));
        }
    }

    // Do as GNU cmp, and completely disable printing if we are
    // outputing to /dev/null.
    #[cfg(not(target_os = "windows"))]
    if is_stdout_dev_null() {
        params.quiet = true;
        params.verbose = false;
        params.print_bytes = false;
    }

    if params.quiet && params.verbose {
        return Err(format!(
            "{executable_str}: options -l and -s are incompatible"
        ));
    }

    params.from = if let Some(from) = from {
        from
    } else if let Some(param) = opts.next() {
        param
    } else {
        return Err(usage_string(&executable_str));
    };
    params.to = if let Some(to) = to {
        to
    } else if let Some(param) = opts.next() {
        param
    } else {
        OsString::from("-")
    };

    // GNU cmp ignores positional skip arguments if -i is provided.
    if params.skip_a.is_none() {
        if skip_pos1.is_some() {
            params.skip_a = skip_pos1;
        } else if let Some(param) = opts.next() {
            let param_str = param.to_string_lossy().to_string();
            params.skip_a = Some(parse_skip(&param_str, &param_str)?);
        }
    };
    if params.skip_b.is_none() {
        if skip_pos2.is_some() {
            params.skip_b = skip_pos2;
        } else if let Some(param) = opts.next() {
            let param_str = param.to_string_lossy().to_string();
            params.skip_b = Some(parse_skip(&param_str, &param_str)?);
        }
    }

    Ok(params)
}

fn prepare_reader(
    path: &OsString,
    skip: &Option<usize>,
    params: &Params,
) -> Result<Box<dyn BufRead>, String> {
    let mut reader: Box<dyn BufRead> = if path == "-" {
        Box::new(BufReader::new(io::stdin()))
    } else {
        match fs::File::open(path) {
            Ok(file) => Box::new(BufReader::new(file)),
            Err(e) => {
                return Err(format_failure_to_read_input_file(
                    &params.executable,
                    path,
                    &e,
                ));
            }
        }
    };

    if let Some(skip) = skip {
        if let Err(e) = io::copy(&mut reader.by_ref().take(*skip as u64), &mut io::sink()) {
            return Err(format_failure_to_read_input_file(
                &params.executable,
                path,
                &e,
            ));
        }
    }

    Ok(reader)
}

#[derive(Debug)]
pub enum Cmp {
    Equal,
    Different,
}

pub fn cmp(params: &Params) -> Result<Cmp, String> {
    let mut from = prepare_reader(&params.from, &params.skip_a, params)?;
    let mut to = prepare_reader(&params.to, &params.skip_b, params)?;

    let mut offset_width = params.max_bytes.unwrap_or(usize::MAX);

    if let (Ok(a_meta), Ok(b_meta)) = (fs::metadata(&params.from), fs::metadata(&params.to)) {
        #[cfg(not(target_os = "windows"))]
        let (a_size, b_size) = (a_meta.size(), b_meta.size());

        #[cfg(target_os = "windows")]
        let (a_size, b_size) = (a_meta.file_size(), b_meta.file_size());

        // If the files have different sizes, we already know they are not identical. If we have not
        // been asked to show even the first difference, we can quit early.
        if params.quiet && a_size != b_size {
            return Ok(Cmp::Different);
        }

        let smaller = cmp::min(a_size, b_size) as usize;
        offset_width = cmp::min(smaller, offset_width);
    }

    let offset_width = 1 + offset_width.checked_ilog10().unwrap_or(1) as usize;

    // Capacity calc: at_byte width + 2 x 3-byte octal numbers + 2 x 4-byte value + 4 spaces
    let mut output = Vec::<u8>::with_capacity(offset_width + 3 * 2 + 4 * 2 + 4);

    let mut at_byte = 1;
    let mut at_line = 1;
    let mut start_of_line = true;
    let mut stdout = BufWriter::new(io::stdout().lock());
    let mut compare = Cmp::Equal;
    loop {
        // Fill up our buffers.
        let from_buf = match from.fill_buf() {
            Ok(buf) => buf,
            Err(e) => {
                return Err(format_failure_to_read_input_file(
                    &params.executable,
                    &params.from,
                    &e,
                ));
            }
        };

        let to_buf = match to.fill_buf() {
            Ok(buf) => buf,
            Err(e) => {
                return Err(format_failure_to_read_input_file(
                    &params.executable,
                    &params.to,
                    &e,
                ));
            }
        };

        // Check for EOF conditions.
        if from_buf.is_empty() && to_buf.is_empty() {
            break;
        }

        if from_buf.is_empty() || to_buf.is_empty() {
            let eof_on = if from_buf.is_empty() {
                &params.from.to_string_lossy()
            } else {
                &params.to.to_string_lossy()
            };

            report_eof(at_byte, at_line, start_of_line, eof_on, params);
            return Ok(Cmp::Different);
        }

        // Fast path - for long files in which almost all bytes are the same we
        // can do a direct comparison to let the compiler optimize.
        let consumed = std::cmp::min(from_buf.len(), to_buf.len());
        if from_buf[..consumed] == to_buf[..consumed] {
            let last = from_buf[..consumed].last().unwrap();

            at_byte += consumed;
            at_line += from_buf[..consumed].iter().filter(|&c| *c == b'\n').count();

            start_of_line = *last == b'\n';

            if let Some(max_bytes) = params.max_bytes {
                if at_byte > max_bytes {
                    break;
                }
            }

            from.consume(consumed);
            to.consume(consumed);

            continue;
        }

        // Iterate over the buffers, the zip iterator will stop us as soon as the
        // first one runs out.
        for (&from_byte, &to_byte) in from_buf.iter().zip(to_buf.iter()) {
            if from_byte != to_byte {
                compare = Cmp::Different;

                if params.verbose {
                    format_verbose_difference(
                        from_byte,
                        to_byte,
                        at_byte,
                        offset_width,
                        &mut output,
                        params,
                    )?;
                    stdout.write_all(output.as_slice()).map_err(|e| {
                        format!(
                            "{}: error printing output: {e}",
                            params.executable.to_string_lossy()
                        )
                    })?;
                    output.clear();
                } else {
                    report_difference(from_byte, to_byte, at_byte, at_line, params);
                    return Ok(Cmp::Different);
                }
            }

            start_of_line = from_byte == b'\n';
            if start_of_line {
                at_line += 1;
            }

            at_byte += 1;

            if let Some(max_bytes) = params.max_bytes {
                if at_byte > max_bytes {
                    break;
                }
            }
        }

        // Notify our readers about the bytes we went over.
        from.consume(consumed);
        to.consume(consumed);
    }

    Ok(compare)
}

// Exit codes are documented at
// https://www.gnu.org/software/diffutils/manual/html_node/Invoking-cmp.html
//     An exit status of 0 means no differences were found,
//     1 means some differences were found,
//     and 2 means trouble.
pub fn main(opts: Peekable<ArgsOs>) -> ExitCode {
    let params = match parse_params(opts) {
        Ok(param) => param,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };

    if params.from == "-" && params.to == "-"
        || same_file::is_same_file(&params.from, &params.to).unwrap_or(false)
    {
        return ExitCode::SUCCESS;
    }

    match cmp(&params) {
        Ok(Cmp::Equal) => ExitCode::SUCCESS,
        Ok(Cmp::Different) => ExitCode::from(1),
        Err(e) => {
            if !params.quiet {
                eprintln!("{e}");
            }
            ExitCode::from(2)
        }
    }
}

#[inline]
fn format_octal(byte: u8, buf: &mut [u8; 3]) -> &str {
    *buf = [b' ', b' ', b'0'];

    let mut num = byte;
    let mut idx = 2; // Start at the last position in the buffer

    // Generate octal digits
    while num > 0 {
        buf[idx] = b'0' + num % 8;
        num /= 8;
        idx = idx.saturating_sub(1);
    }

    // SAFETY: the operations we do above always land within ascii range.
    unsafe { std::str::from_utf8_unchecked(&buf[..]) }
}

#[inline]
fn write_visible_byte(output: &mut Vec<u8>, byte: u8) -> usize {
    match byte {
        // Control characters: ^@, ^A, ..., ^_
        0..=31 => {
            output.push(b'^');
            output.push(byte + 64);
            2
        }
        // Printable ASCII (space through ~)
        32..=126 => {
            output.push(byte);
            1
        }
        // DEL: ^?
        127 => {
            output.extend_from_slice(b"^?");
            2
        }
        // High bytes with control equivalents: M-^@, M-^A, ..., M-^_
        128..=159 => {
            output.push(b'M');
            output.push(b'-');
            output.push(b'^');
            output.push(byte - 64);
            4
        }
        // High bytes: M-<space>, M-!, ..., M-~
        160..=254 => {
            output.push(b'M');
            output.push(b'-');
            output.push(byte - 128);
            3
        }
        // Byte 255: M-^?
        255 => {
            output.extend_from_slice(b"M-^?");
            4
        }
    }
}

/// Writes a byte in visible form with right-padding to 4 spaces.
#[inline]
fn write_visible_byte_padded(output: &mut Vec<u8>, byte: u8) {
    const SPACES: &[u8] = b"    ";
    const WIDTH: usize = SPACES.len();

    let display_width = write_visible_byte(output, byte);

    // Add right-padding spaces
    let padding = WIDTH.saturating_sub(display_width);
    output.extend_from_slice(&SPACES[..padding]);
}

/// Formats a byte as a visible string (for non-performance-critical path)
#[inline]
fn format_visible_byte(byte: u8) -> String {
    let mut result = Vec::with_capacity(4);
    write_visible_byte(&mut result, byte);
    // SAFETY: the checks and shifts in write_visible_byte match what cat and GNU
    // cmp do to ensure characters fall inside the ascii range.
    unsafe { String::from_utf8_unchecked(result) }
}

// This function has been optimized to not use the Rust fmt system, which
// leads to a massive speed up when processing large files: cuts the time
// for comparing 2 ~36MB completely different files in half on an M1 Max.
#[inline]
fn format_verbose_difference(
    from_byte: u8,
    to_byte: u8,
    at_byte: usize,
    offset_width: usize,
    output: &mut Vec<u8>,
    params: &Params,
) -> Result<(), String> {
    assert!(!params.quiet);

    let mut at_byte_buf = itoa::Buffer::new();
    let mut from_oct = [0u8; 3]; // for octal conversions
    let mut to_oct = [0u8; 3];

    if params.print_bytes {
        // "{:>width$} {:>3o} {:4} {:>3o} {}",
        let at_byte_str = at_byte_buf.format(at_byte);
        let at_byte_padding = offset_width.saturating_sub(at_byte_str.len());

        for _ in 0..at_byte_padding {
            output.push(b' ')
        }

        output.extend_from_slice(at_byte_str.as_bytes());

        output.push(b' ');

        output.extend_from_slice(format_octal(from_byte, &mut from_oct).as_bytes());

        output.push(b' ');

        write_visible_byte_padded(output, from_byte);

        output.push(b' ');

        output.extend_from_slice(format_octal(to_byte, &mut to_oct).as_bytes());

        output.push(b' ');

        write_visible_byte(output, to_byte);

        output.push(b'\n');
    } else {
        // "{:>width$} {:>3o} {:>3o}"
        let at_byte_str = at_byte_buf.format(at_byte);
        let at_byte_padding = offset_width - at_byte_str.len();

        for _ in 0..at_byte_padding {
            output.push(b' ')
        }

        output.extend_from_slice(at_byte_str.as_bytes());

        output.push(b' ');

        output.extend_from_slice(format_octal(from_byte, &mut from_oct).as_bytes());

        output.push(b' ');

        output.extend_from_slice(format_octal(to_byte, &mut to_oct).as_bytes());

        output.push(b'\n');
    }

    Ok(())
}

#[inline]
fn report_eof(at_byte: usize, at_line: usize, start_of_line: bool, eof_on: &str, params: &Params) {
    if params.quiet {
        return;
    }

    if at_byte == 1 {
        eprintln!(
            "{}: EOF on '{}' which is empty",
            params.executable.to_string_lossy(),
            eof_on
        );
    } else if params.verbose {
        eprintln!(
            "{}: EOF on '{}' after byte {}",
            params.executable.to_string_lossy(),
            eof_on,
            at_byte - 1,
        );
    } else if start_of_line {
        eprintln!(
            "{}: EOF on '{}' after byte {}, line {}",
            params.executable.to_string_lossy(),
            eof_on,
            at_byte - 1,
            at_line - 1
        );
    } else {
        eprintln!(
            "{}: EOF on '{}' after byte {}, in line {}",
            params.executable.to_string_lossy(),
            eof_on,
            at_byte - 1,
            at_line
        );
    }
}

fn is_posix_locale() -> bool {
    let locale = if let Ok(locale) = env::var("LC_ALL") {
        locale
    } else if let Ok(locale) = env::var("LC_MESSAGES") {
        locale
    } else if let Ok(locale) = env::var("LANG") {
        locale
    } else {
        "C".to_string()
    };

    locale == "C" || locale == "POSIX"
}

#[inline]
fn report_difference(from_byte: u8, to_byte: u8, at_byte: usize, at_line: usize, params: &Params) {
    if params.quiet {
        return;
    }

    let term = if is_posix_locale() && !params.print_bytes {
        "char"
    } else {
        "byte"
    };
    print!(
        "{} {} differ: {term} {}, line {}",
        &params.from.to_string_lossy(),
        &params.to.to_string_lossy(),
        at_byte,
        at_line
    );
    if params.print_bytes {
        let char_width = if to_byte >= 0x7F { 2 } else { 1 };
        print!(
            " is {:>3o} {:char_width$} {:>3o} {:char_width$}",
            from_byte,
            format_visible_byte(from_byte),
            to_byte,
            format_visible_byte(to_byte)
        );
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn os(s: &str) -> OsString {
        OsString::from(s)
    }

    #[test]
    fn positional() {
        assert_eq!(
            Ok(Params {
                executable: os("cmp"),
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params([os("cmp"), os("foo"), os("bar")].iter().cloned().peekable())
        );

        assert_eq!(
            Ok(Params {
                executable: os("cmp"),
                from: os("foo"),
                to: os("-"),
                ..Default::default()
            }),
            parse_params([os("cmp"), os("foo")].iter().cloned().peekable())
        );

        assert_eq!(
            Ok(Params {
                executable: os("cmp"),
                from: os("foo"),
                to: os("--help"),
                ..Default::default()
            }),
            parse_params(
                [os("cmp"), os("foo"), os("--"), os("--help")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );

        assert_eq!(
            Ok(Params {
                executable: os("cmp"),
                from: os("foo"),
                to: os("bar"),
                skip_a: Some(1),
                skip_b: None,
                ..Default::default()
            }),
            parse_params(
                [os("cmp"), os("foo"), os("bar"), os("1")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );

        assert_eq!(
            Ok(Params {
                executable: os("cmp"),
                from: os("foo"),
                to: os("bar"),
                skip_a: Some(1),
                skip_b: Some(usize::MAX),
                ..Default::default()
            }),
            parse_params(
                [os("cmp"), os("foo"), os("bar"), os("1"), os("2Y")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );

        // Bad positional arguments.
        assert_eq!(
            Err("Usage: cmp <from> <to>".to_string()),
            parse_params(
                [os("cmp"), os("foo"), os("bar"), os("1"), os("2"), os("3")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Err("Usage: cmp <from> <to>".to_string()),
            parse_params([os("cmp")].iter().cloned().peekable())
        );
    }

    #[test]
    fn execution_modes() {
        let print_bytes = Params {
            executable: os("cmp"),
            from: os("foo"),
            to: os("bar"),
            print_bytes: true,
            ..Default::default()
        };
        assert_eq!(
            Ok(print_bytes.clone()),
            parse_params(
                [os("cmp"), os("-b"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(print_bytes),
            parse_params(
                [os("cmp"), os("--print-bytes"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );

        let verbose = Params {
            executable: os("cmp"),
            from: os("foo"),
            to: os("bar"),
            verbose: true,
            ..Default::default()
        };
        assert_eq!(
            Ok(verbose.clone()),
            parse_params(
                [os("cmp"), os("-l"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(verbose),
            parse_params(
                [os("cmp"), os("--verbose"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );

        let verbose_and_print_bytes = Params {
            executable: os("cmp"),
            from: os("foo"),
            to: os("bar"),
            print_bytes: true,
            verbose: true,
            ..Default::default()
        };
        assert_eq!(
            Ok(verbose_and_print_bytes.clone()),
            parse_params(
                [os("cmp"), os("-l"), os("-b"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(verbose_and_print_bytes.clone()),
            parse_params(
                [os("cmp"), os("-lb"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(verbose_and_print_bytes),
            parse_params(
                [os("cmp"), os("-bl"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );

        assert_eq!(
            Ok(Params {
                executable: os("cmp"),
                from: os("foo"),
                to: os("bar"),
                quiet: true,
                ..Default::default()
            }),
            parse_params(
                [os("cmp"), os("-s"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );

        // Some options do not mix.
        assert_eq!(
            Err("cmp: options -l and -s are incompatible".to_string()),
            parse_params(
                [os("cmp"), os("-l"), os("-s"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
    }

    #[test]
    fn max_bytes() {
        let max_bytes = Params {
            executable: os("cmp"),
            from: os("foo"),
            to: os("bar"),
            max_bytes: Some(1),
            ..Default::default()
        };
        assert_eq!(
            Ok(max_bytes.clone()),
            parse_params(
                [os("cmp"), os("-n"), os("1"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(max_bytes),
            parse_params(
                [os("cmp"), os("--bytes=1"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );

        assert_eq!(
            Ok(Params {
                executable: os("cmp"),
                from: os("foo"),
                to: os("bar"),
                max_bytes: Some(usize::MAX),
                ..Default::default()
            }),
            parse_params(
                [
                    os("cmp"),
                    os("--bytes=99999999999999999999999999999999999999999999999999999999999"),
                    os("foo"),
                    os("bar")
                ]
                .iter()
                .cloned()
                .peekable()
            )
        );

        // Failure case
        assert_eq!(
            Err("cmp: invalid --bytes value '1K'".to_string()),
            parse_params(
                [os("cmp"), os("--bytes=1K"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
    }

    #[test]
    fn skips() {
        let skips = Params {
            executable: os("cmp"),
            from: os("foo"),
            to: os("bar"),
            skip_a: Some(1),
            skip_b: Some(1),
            ..Default::default()
        };
        assert_eq!(
            Ok(skips.clone()),
            parse_params(
                [os("cmp"), os("-i"), os("1"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(skips),
            parse_params(
                [os("cmp"), os("--ignore-initial=1"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );

        assert_eq!(
            Ok(Params {
                executable: os("cmp"),
                from: os("foo"),
                to: os("bar"),
                skip_a: Some(usize::MAX),
                skip_b: Some(usize::MAX),
                ..Default::default()
            }),
            parse_params(
                [
                    os("cmp"),
                    os("-i"),
                    os("99999999999999999999999999999999999999999999999999999999999"),
                    os("foo"),
                    os("bar")
                ]
                .iter()
                .cloned()
                .peekable()
            )
        );

        assert_eq!(
            Ok(Params {
                executable: os("cmp"),
                from: os("foo"),
                to: os("bar"),
                skip_a: Some(1),
                skip_b: Some(2),
                ..Default::default()
            }),
            parse_params(
                [os("cmp"), os("--ignore-initial=1:2"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );

        assert_eq!(
            Ok(Params {
                executable: os("cmp"),
                from: os("foo"),
                to: os("bar"),
                skip_a: Some(1_000_000_000),
                #[cfg(target_pointer_width = "32")]
                skip_b: Some((2_147_483_647.5 * 2.0) as usize),
                #[cfg(target_pointer_width = "64")]
                skip_b: Some(1_152_921_504_606_846_976 * 2),
                ..Default::default()
            }),
            parse_params(
                [
                    os("cmp"),
                    os("--ignore-initial=1GB:2E"),
                    os("foo"),
                    os("bar")
                ]
                .iter()
                .cloned()
                .peekable()
            )
        );

        // All special suffixes.
        for (i, suffixes) in [
            ["kB", "K"],
            ["MB", "M"],
            ["GB", "G"],
            ["TB", "T"],
            ["PB", "P"],
            ["EB", "E"],
            ["ZB", "Z"],
            ["YB", "Y"],
        ]
        .iter()
        .enumerate()
        {
            let values = [
                1_000usize.checked_pow((i + 1) as u32).unwrap_or(usize::MAX),
                1024usize.checked_pow((i + 1) as u32).unwrap_or(usize::MAX),
            ];
            for (j, v) in values.iter().enumerate() {
                assert_eq!(
                    Ok(Params {
                        executable: os("cmp"),
                        from: os("foo"),
                        to: os("bar"),
                        skip_a: Some(*v),
                        skip_b: Some(2),
                        ..Default::default()
                    }),
                    parse_params(
                        [
                            os("cmp"),
                            os("-i"),
                            os(&format!("1{}:2", suffixes[j])),
                            os("foo"),
                            os("bar"),
                        ]
                        .iter()
                        .cloned()
                        .peekable()
                    )
                );
            }
        }

        // Ignores positional arguments when -i is provided.
        assert_eq!(
            Ok(Params {
                executable: os("cmp"),
                from: os("foo"),
                to: os("bar"),
                skip_a: Some(1),
                skip_b: Some(2),
                ..Default::default()
            }),
            parse_params(
                [
                    os("cmp"),
                    os("-i"),
                    os("1:2"),
                    os("foo"),
                    os("bar"),
                    os("3"),
                    os("4")
                ]
                .iter()
                .cloned()
                .peekable()
            )
        );

        // Failure cases
        assert_eq!(
            Err("cmp: invalid --ignore-initial value '1mb'".to_string()),
            parse_params(
                [os("cmp"), os("--ignore-initial=1mb"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Err("cmp: invalid --ignore-initial value '1:2:3'".to_string()),
            parse_params(
                [
                    os("cmp"),
                    os("--ignore-initial=1:2:3"),
                    os("foo"),
                    os("bar")
                ]
                .iter()
                .cloned()
                .peekable()
            )
        );
        assert_eq!(
            Err("cmp: invalid --ignore-initial value '-1'".to_string()),
            parse_params(
                [os("cmp"), os("--ignore-initial=-1"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
    }
}
