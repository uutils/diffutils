// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

// spell-checker:ignore ilog

pub mod params_cmp;

use std::env::{self};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::{cmp, fs, io};

use clap::Command;
use uudiff::common_errors::UtilsError;
use uudiff::error::{FromIo, UResult};
use uudiff::utils::{self, CompareOk};

use crate::params_cmp::{BytesLimitU64, Params, SkipU64};

#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;

#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::MetadataExt;

/// Entry into cmp.
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uudiff::clap_localization::handle_clap_result_with_exit_code(uu_app(), args, 2)?;

    let params: Params = matches.try_into()?;

    match cmp_compare(&params) {
        Ok(res) => match res {
            CompareOk::Equal => uucore::error::set_exit_code(0),
            CompareOk::Different => uucore::error::set_exit_code(1),
        },
        Err(e) => {
            // dbg!(&params, &e);
            if params.silent {
                uucore::error::set_exit_code(2);
                return Ok(());
            }
            return Err(e);
        }
    }

    Ok(())
}

pub fn cmp_compare(params: &Params) -> UResult<CompareOk> {
    // check if file is actually a directory, which is not allowed
    if params.from != "-" {
        match fs::metadata(&params.from) {
            Ok(m) => {
                if m.is_dir() {
                    return Err(UtilsError::DirectoryNotAllowed(params.from.clone()).into());
                }
            }
            Err(e) => {
                let io = e.map_err_context(|| params.from_as_string_lossy());
                return Err(UtilsError::Io(io).into());
            }
        }
    }
    if params.to != "-" {
        match fs::metadata(&params.to) {
            Ok(m) => {
                if m.is_dir() {
                    return Err(UtilsError::DirectoryNotAllowed(params.to.clone()).into());
                }
            }
            Err(e) => {
                let io = e.map_err_context(|| params.to_as_string_lossy());
                return Err(UtilsError::Io(io).into());
            }
        }
    }
    // check is same file and has no shift by skipping bytes
    if utils::is_same_file(&params.from, &params.to)
        && params.skip_bytes_from == params.skip_bytes_to
    {
        return Ok(CompareOk::Equal);
    }

    let mut from = prepare_reader(&params.from, params.skip_bytes_from)?;
    let mut to = prepare_reader(&params.to, params.skip_bytes_to)?;

    let mut offset_width = params.bytes_limit.unwrap_or(BytesLimitU64::MAX);

    if let (Ok(a_meta), Ok(b_meta)) = (fs::metadata(&params.from), fs::metadata(&params.to)) {
        #[cfg(not(target_os = "windows"))]
        let (from_size, to_size) = (a_meta.size(), b_meta.size());

        #[cfg(target_os = "windows")]
        let (from_size, to_size) = (a_meta.file_size(), b_meta.file_size());

        // If the files have different sizes, we already know they are not identical. If we have not
        // been asked to show even the first difference, we can quit early.
        if params.silent && from_size != to_size {
            return Ok(CompareOk::Different);
        }

        let smaller = cmp::min(from_size, to_size);
        offset_width = cmp::min(smaller, offset_width);
    }

    let offset_width = 1 + offset_width.checked_ilog10().unwrap_or(1) as usize;

    // Capacity calc: at_byte width + 2 x 3-byte octal numbers + 2 x 4-byte value + 4 spaces
    let mut output = Vec::<u8>::with_capacity(offset_width + 3 * 2 + 4 * 2 + 4);

    let mut at_byte = 1;
    let mut at_line = 1;
    let mut start_of_line = true;
    let mut stdout = BufWriter::new(io::stdout().lock());
    let mut compare = CompareOk::Equal;
    loop {
        // Fill up our buffers.
        let from_buf = match from.fill_buf() {
            Ok(buf) => buf,
            Err(e) => {
                let io = e.map_err_context(|| params.from_as_string_lossy());
                return Err(UtilsError::Io(io).into());
            }
        };

        let to_buf = match to.fill_buf() {
            Ok(buf) => buf,
            Err(e) => {
                let io = e.map_err_context(|| params.to_as_string_lossy());
                return Err(UtilsError::Io(io).into());
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
            return Ok(CompareOk::Different);
        }

        // Fast path - for long files in which almost all bytes are the same we
        // can do a direct comparison to let the compiler optimize.
        let consumed = std::cmp::min(from_buf.len(), to_buf.len());
        if from_buf[..consumed] == to_buf[..consumed] {
            let last = from_buf[..consumed].last().unwrap();

            at_byte += consumed as BytesLimitU64;
            // at_line += from_buf[..consumed].iter().filter(|&c| *c == b'\n').count() as u64;
            at_line += bytecount::count(&from_buf[..consumed], b'\n') as u64;

            start_of_line = *last == b'\n';

            if let Some(bytes_limit) = params.bytes_limit {
                if at_byte > bytes_limit {
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
                compare = CompareOk::Different;

                if params.verbose {
                    format_verbose_difference(
                        from_byte,
                        to_byte,
                        at_byte,
                        offset_width,
                        &mut output,
                        params,
                    );
                    // TODO test error returns exit code 2
                    stdout.write_all(output.as_slice())?;
                    // if let Err(e) = stdout.write_all(output.as_slice())
                    // // .map_err(|e| format!("{}: error printing output: {e}", uucore::util_name()))
                    // {
                    //     return Err(CmpError::FileIo("stdout".into(), e));
                    // }
                    output.clear();
                } else {
                    report_difference(from_byte, to_byte, at_byte, at_line, params)?;
                    return Ok(CompareOk::Different);
                }
            }

            start_of_line = from_byte == b'\n';
            if start_of_line {
                at_line += 1;
            }

            at_byte += 1;

            if let Some(max_bytes) = params.bytes_limit {
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

fn prepare_reader(
    path: &OsString,
    ignore_initial: Option<SkipU64>,
) -> Result<Box<dyn BufRead>, UtilsError> {
    let mut reader: Box<dyn BufRead> = if path == "-" {
        Box::new(BufReader::new(io::stdin()))
    } else {
        match fs::File::open(path) {
            Ok(file) => Box::new(BufReader::new(file)),
            Err(e) => {
                let io = e.map_err_context(|| path.to_string_lossy().to_string());
                return Err(UtilsError::Io(io));
            }
        }
    };

    #[allow(clippy::collapsible_if)]
    if let Some(skip) = ignore_initial {
        if let Err(e) = io::copy(&mut reader.by_ref().take(skip), &mut io::sink()) {
            let io = e.map_err_context(|| path.to_string_lossy().to_string());
            return Err(UtilsError::Io(io));
        }
    }

    Ok(reader)
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

// This function has been optimized to not use the Rust fmt system, which
// leads to a massive speed up when processing large files: cuts the time
// for comparing 2 ~36MB completely different files in half on an M1 Max.
#[inline]
fn format_verbose_difference(
    from_byte: u8,
    to_byte: u8,
    at_byte: BytesLimitU64,
    offset_width: usize,
    output: &mut Vec<u8>,
    params: &Params,
) {
    assert!(!params.silent);

    let mut at_byte_buf = itoa::Buffer::new();
    let mut from_oct = [0u8; 3]; // for octal conversions
    let mut to_oct = [0u8; 3];

    if params.print_bytes {
        // "{:>width$} {:>3o} {:4} {:>3o} {}",
        let at_byte_str = at_byte_buf.format(at_byte);
        let at_byte_padding = offset_width.saturating_sub(at_byte_str.len());

        for _ in 0..at_byte_padding {
            output.push(b' ');
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
            output.push(b' ');
        }

        output.extend_from_slice(at_byte_str.as_bytes());

        output.push(b' ');

        output.extend_from_slice(format_octal(from_byte, &mut from_oct).as_bytes());

        output.push(b' ');

        output.extend_from_slice(format_octal(to_byte, &mut to_oct).as_bytes());

        output.push(b'\n');
    }
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
fn report_difference(
    from_byte: u8,
    to_byte: u8,
    at_byte: BytesLimitU64,
    at_line: u64,
    params: &Params,
) -> io::Result<()> {
    if params.silent {
        return Ok(());
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
    // Instead of println!(), which panics in case of error (> /dev/full).
    let mut stdout = io::stdout();
    writeln!(stdout)?;
    stdout.flush()?;

    Ok(())
}

#[inline]
fn report_eof(
    at_byte: BytesLimitU64,
    at_line: u64,
    start_of_line: bool,
    eof_on: &str,
    params: &Params,
) {
    if params.silent {
        return;
    }

    if at_byte == 1 {
        eprintln!(
            "{}: EOF on '{}' which is empty",
            uucore::util_name(),
            eof_on
        );
    } else if params.verbose {
        eprintln!(
            "{}: EOF on '{}' after byte {}",
            uucore::util_name(),
            eof_on,
            at_byte - 1,
        );
    } else if start_of_line {
        eprintln!(
            "{}: EOF on '{}' after byte {}, line {}",
            uucore::util_name(),
            eof_on,
            at_byte - 1,
            at_line - 1
        );
    } else {
        eprintln!(
            "{}: EOF on '{}' after byte {}, in line {}",
            uucore::util_name(),
            eof_on,
            at_byte - 1,
            at_line
        );
    }
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

// Required for build.rs
pub fn uu_app() -> Command {
    params_cmp::uu_app()
}
