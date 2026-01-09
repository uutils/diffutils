// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use regex::Regex;

// use crate::params::parse_params;
use crate::side_diff;
use crate::utils;
use std::env::ArgsOs;
use std::ffi::OsString;
use std::io::{self, stdout, Write};
use std::iter::Peekable;
use std::process::{exit, ExitCode};

#[derive(Eq, PartialEq, Debug)]
pub struct Params {
    pub executable: OsString,
    pub from: OsString,
    pub to: OsString,
    pub expand_tabs: bool,
    pub tabsize: usize,
    pub width: usize,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            executable: OsString::default(),
            from: OsString::default(),
            to: OsString::default(),
            expand_tabs: false,
            tabsize: 8,
            width: 130,
        }
    }
}

pub fn parse_params<I: Iterator<Item = OsString>>(mut opts: Peekable<I>) -> Result<Params, String> {
    let Some(executable) = opts.next() else {
        return Err("Usage: <exe> <from> <to>".to_string());
    };

    let mut params = Params {
        executable,
        ..Default::default()
    };

    let mut from = None;
    let mut to = None;
    let tabsize_re = Regex::new(r"^--tabsize=(?<num>\d+)$").unwrap();
    let width_re = Regex::new(r"--width=(?P<long>\d+)$").unwrap();

    while let Some(param) = opts.next() {
        if param == "-" {
            if from.is_none() {
                from = Some(param);
            } else if to.is_none() {
                to = Some(param);
            } else {
                return Err(format!(
                    "Usage: {} <from> <to>",
                    params.executable.to_string_lossy()
                ));
            }
            continue;
        }

        if param == "-t" || param == "--expand-tabs" {
            params.expand_tabs = true;
            continue;
        }

        if tabsize_re.is_match(param.to_string_lossy().as_ref()) {
            // Because param matches the regular expression,
            // it is safe to assume it is valid UTF-8.
            let param = param.into_string().unwrap();
            let tabsize_str = tabsize_re
                .captures(param.as_str())
                .unwrap()
                .name("num")
                .unwrap()
                .as_str();
            params.tabsize = match tabsize_str.parse::<usize>() {
                Ok(num) => {
                    if num == 0 {
                        return Err("invalid tabsize «0»".to_string());
                    }

                    num
                }
                Err(_) => return Err(format!("invalid tabsize «{tabsize_str}»")),
            };

            continue;
        }

        if width_re.is_match(param.to_string_lossy().as_ref()) {
            let param = param.into_string().unwrap();
            let width_str: &str = width_re
                .captures(param.as_str())
                .unwrap()
                .name("long")
                .unwrap()
                .as_str();

            params.width = match width_str.parse::<usize>() {
                Ok(num) => {
                    if num == 0 {
                        return Err("invalid width «0»".to_string());
                    }

                    num
                }
                Err(_) => return Err(format!("invalid width «{width_str}»")),
            };
            continue;
        }

        if from.is_none() {
            from = Some(param);
        } else if to.is_none() {
            to = Some(param);
        } else {
            return Err(format!(
                "Usage: {} <from> <to>",
                params.executable.to_string_lossy()
            ));
        }
    }

    params.from = if let Some(from) = from {
        from
    } else if let Some(param) = opts.next() {
        param
    } else {
        return Err(format!("Err"));
    };

    params.to = if let Some(to) = to {
        to
    } else if let Some(param) = opts.next() {
        param
    } else {
        return Err(format!("Err"));
    };

    Ok(params)
}

pub fn main(opts: Peekable<ArgsOs>) -> ExitCode {
    let params = parse_params(opts).unwrap_or_else(|error| {
        eprintln!("{error}");
        exit(2);
    });

    if params.from == "-" && params.to == "-"
        || same_file::is_same_file(&params.from, &params.to).unwrap_or(false)
    {
        return ExitCode::SUCCESS;
    }

    let (from_content, to_content) = match utils::read_both_files(&params.from, &params.to) {
        Ok(contents) => contents,
        Err((filepath, error)) => {
            eprintln!(
                "{}",
                utils::format_failure_to_read_input_file(&params.executable, &filepath, &error)
            );
            return ExitCode::from(2);
        }
    };

    // run diff
    let mut output = stdout().lock();
    let result = side_diff::diff(
        &from_content,
        &to_content,
        &mut output,
        &side_diff::Params {
            tabsize: params.tabsize,
            width: params.width,
            expand_tabs: params.expand_tabs,
        },
    );

    io::stdout().write_all(&result).unwrap();
    if result.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> OsString {
        OsString::from(s)
    }

    #[test]
    fn sdiff_params() {
        assert_eq!(
            Ok(Params {
                executable: os("sdiff"),
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params(
                [os("sdiff"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        )
    }
}
