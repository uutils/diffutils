// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::{ffi::OsString, io::Write};

use regex::Regex;
use unicode_width::UnicodeWidthStr;

/// Replace tabs by spaces in the input line.
/// Correctly handle multi-bytes characters.
/// This assumes that line does not contain any line breaks (if it does, the result is undefined).
#[must_use]
pub fn do_expand_tabs(line: &[u8], tabsize: usize) -> Vec<u8> {
    let tab = b'\t';
    let ntabs = line.iter().filter(|c| **c == tab).count();
    if ntabs == 0 {
        return line.to_vec();
    }
    let mut result = Vec::with_capacity(line.len() + ntabs * (tabsize - 1));
    let mut offset = 0;

    let mut iter = line.split(|c| *c == tab).peekable();
    while let Some(chunk) = iter.next() {
        match String::from_utf8(chunk.to_vec()) {
            Ok(s) => offset += UnicodeWidthStr::width(s.as_str()),
            Err(_) => offset += chunk.len(),
        }
        result.extend_from_slice(chunk);
        if iter.peek().is_some() {
            result.resize(result.len() + tabsize - offset % tabsize, b' ');
            offset = 0;
        }
    }

    result
}

/// Write a single line to an output stream, expanding tabs to space if necessary.
/// This assumes that line does not contain any line breaks
/// (if it does and tabs are to be expanded to spaces, the result is undefined).
pub fn do_write_line(
    output: &mut Vec<u8>,
    line: &[u8],
    expand_tabs: bool,
    tabsize: usize,
) -> std::io::Result<()> {
    if expand_tabs {
        output.write_all(do_expand_tabs(line, tabsize).as_slice())
    } else {
        output.write_all(line)
    }
}

/// Retrieves the modification time of the input file specified by file path
/// If an error occurs, it returns the current system time
pub fn get_modification_time(file_path: &str) -> String {
    use chrono::{DateTime, Local};
    use std::fs;
    use std::time::SystemTime;

    let modification_time: SystemTime = fs::metadata(file_path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::now());

    let modification_time: DateTime<Local> = modification_time.into();
    let modification_time: String = modification_time
        .format("%Y-%m-%d %H:%M:%S%.9f %z")
        .to_string();

    modification_time
}

pub fn report_failure_to_read_input_file(
    executable: &OsString,
    filepath: &OsString,
    error: &std::io::Error,
) {
    // std::io::Error's display trait outputs "{detail} (os error {code})"
    // but we want only the {detail} (error string) part
    let error_code_re = Regex::new(r"\ \(os\ error\ \d+\)$").unwrap();
    eprintln!(
        "{}: {}: {}",
        executable.to_string_lossy(),
        filepath.to_string_lossy(),
        error_code_re.replace(error.to_string().as_str(), ""),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    mod expand_tabs {
        use super::*;
        use pretty_assertions::assert_eq;

        fn assert_tab_expansion(line: &str, tabsize: usize, expected: &str) {
            assert_eq!(
                do_expand_tabs(line.as_bytes(), tabsize),
                expected.as_bytes()
            );
        }

        #[test]
        fn basics() {
            assert_tab_expansion("foo barr   baz", 8, "foo barr   baz");
            assert_tab_expansion("foo\tbarr\tbaz", 8, "foo     barr    baz");
            assert_tab_expansion("foo\tbarr\tbaz", 5, "foo  barr baz");
            assert_tab_expansion("foo\tbarr\tbaz", 2, "foo barr  baz");
        }

        #[test]
        fn multibyte_chars() {
            assert_tab_expansion("foo\tépée\tbaz", 8, "foo     épée    baz");
            assert_tab_expansion("foo\t😉\tbaz", 5, "foo  😉   baz");

            // Note: The Woman Scientist emoji (👩‍🔬) is a ZWJ sequence combining
            // the Woman emoji (👩) and the Microscope emoji (🔬). On supported platforms
            // it is displayed as a single emoji and has a print size of 2 columns.
            // Terminal emulators tend to not support this, and display the two emojis
            // side by side, thus accounting for a print size of 4 columns, but the
            // unicode_width crate reports a correct size of 2.
            assert_tab_expansion("foo\t👩‍🔬\tbaz", 6, "foo   👩‍🔬    baz");
        }

        #[test]
        fn invalid_utf8() {
            // [240, 240, 152, 137] is an invalid UTF-8 sequence, so it is handled as 4 bytes
            assert_eq!(
                do_expand_tabs(&[240, 240, 152, 137, 9, 102, 111, 111], 8),
                &[240, 240, 152, 137, 32, 32, 32, 32, 102, 111, 111]
            );
        }
    }

    mod write_line {
        use super::*;
        use pretty_assertions::assert_eq;

        fn assert_line_written(line: &str, expand_tabs: bool, tabsize: usize, expected: &str) {
            let mut output: Vec<u8> = Vec::new();
            assert!(do_write_line(&mut output, line.as_bytes(), expand_tabs, tabsize).is_ok());
            assert_eq!(output, expected.as_bytes());
        }

        #[test]
        fn basics() {
            assert_line_written("foo bar baz", false, 8, "foo bar baz");
            assert_line_written("foo bar\tbaz", false, 8, "foo bar\tbaz");
            assert_line_written("foo bar\tbaz", true, 8, "foo bar baz");
        }
    }

    mod modification_time {
        use super::*;

        #[test]
        fn set_time() {
            use chrono::{DateTime, Local};
            use std::time::SystemTime;
            use tempfile::NamedTempFile;

            let temp = NamedTempFile::new().unwrap();
            // set file modification time equal to current time
            let current = SystemTime::now();
            let _ = temp.as_file().set_modified(current);

            // format current time
            let current: DateTime<Local> = current.into();
            let current: String = current.format("%Y-%m-%d %H:%M:%S%.9f %z").to_string();

            // verify
            assert_eq!(
                current,
                get_modification_time(&temp.path().to_string_lossy())
            );
        }

        #[test]
        fn invalid_file() {
            use chrono::{DateTime, Local};
            use std::time::SystemTime;

            let invalid_file = "target/utils/invalid-file";

            // store current time before calling `get_modification_time`
            // Because the file is invalid, it will return SystemTime::now()
            // which will be greater than previously saved time
            let current_time: DateTime<Local> = SystemTime::now().into();
            let m_time: DateTime<Local> = get_modification_time(invalid_file).parse().unwrap();

            assert!(m_time > current_time);
        }
    }
}
