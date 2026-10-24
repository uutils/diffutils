use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::iter::Peekable;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::bre::Bre;
use crate::color::ColorWhen;
use crate::utils::fold_file_name_case;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Format {
    #[default]
    Normal,
    Unified,
    Context,
    Ed,
    SideBySide,
}

/// How to handle an operand that does not exist (`-N` and
/// `--unidirectional-new-file`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MissingFile {
    /// A missing operand is an error.
    #[default]
    Error,
    /// A missing operand is treated as an empty file.
    Empty,
    /// Only a missing first operand is treated as an empty file.
    EmptyIfFirst,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Params {
    pub executable: OsString,
    pub from: OsString,
    pub to: OsString,
    pub format: Format,
    pub context_count: usize,
    pub report_identical_files: bool,
    pub brief: bool,
    pub expand_tabs: bool,
    pub tabsize: usize,
    pub width: usize,
    pub missing_file: MissingFile,
    pub text: bool,
    pub color: ColorWhen,
    /// The raw `--palette` value, validated when colors are enabled.
    pub palette: Option<String>,
    pub presume_output_tty: bool,
    /// Basic regular expressions matching the lines shown after hunk
    /// headers (`-F` and `-p`).
    pub function_line_patterns: Vec<Bre>,
    pub recursive: bool,
    /// The name from which to start comparing the top-level directories
    /// (`-S`).
    pub starting_file: Option<OsString>,
    pub ignore_file_name_case: bool,
}

/// The pattern of `-p`, matching the first line of C functions.
const C_FUNCTION_PATTERN: &str = "^[[:alpha:]$_]";

impl Default for Params {
    fn default() -> Self {
        Self {
            executable: OsString::default(),
            from: OsString::default(),
            to: OsString::default(),
            format: Format::default(),
            context_count: 3,
            report_identical_files: false,
            brief: false,
            expand_tabs: false,
            tabsize: 8,
            width: 130,
            missing_file: MissingFile::default(),
            text: false,
            color: ColorWhen::default(),
            palette: None,
            presume_output_tty: false,
            function_line_patterns: Vec::new(),
            recursive: false,
            starting_file: None,
            ignore_file_name_case: false,
        }
    }
}

/// Parses the command line, and also returns its arguments other than the
/// program name and the operands.
pub fn parse_params<I: Iterator<Item = OsString>>(
    opts: Peekable<I>,
) -> Result<(Params, Vec<OsString>), String> {
    let args: Vec<OsString> = opts.collect();
    // The number of arguments taken from `opts`, including a peeked one, to
    // locate the operands.
    let taken = Cell::new(0_usize);
    let mut opts = args
        .iter()
        .cloned()
        .inspect(|_| taken.set(taken.get() + 1))
        .peekable();
    let mut operand_positions = Vec::new();
    let Some(executable) = opts.next() else {
        return Err("Usage: <exe> <from> <to>".to_string());
    };
    let mut params = Params {
        executable,
        ..Default::default()
    };
    let mut from = None;
    let mut to = None;
    let mut format = None;
    let mut context = None;
    let mut show_c_function = false;
    let tabsize_re = Regex::new(r"^--tabsize=(?<num>\d+)$").unwrap();
    let width_re = Regex::new(r"^--width=(?P<long>\d+)$").unwrap();
    while let Some(param) = opts.next() {
        let next_param = opts.peek();
        if param == "--" {
            break;
        }
        if param == "-" {
            operand_positions.push(taken.get() - 1 - usize::from(opts.peek().is_some()));
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
        if param == "-s" || param == "--report-identical-files" {
            params.report_identical_files = true;
            continue;
        }
        if param == "-q" || param == "--brief" {
            params.brief = true;
            continue;
        }
        if param == "-N" || param == "--new-file" {
            params.missing_file = MissingFile::Empty;
            continue;
        }
        if param == "--unidirectional-new-file" {
            if params.missing_file != MissingFile::Empty {
                params.missing_file = MissingFile::EmptyIfFirst;
            }
            continue;
        }
        if param == "--color" {
            params.color = ColorWhen::Auto;
            continue;
        }
        if let Some(when) = param.to_str().and_then(|p| p.strip_prefix("--color=")) {
            params.color = match when {
                "auto" => ColorWhen::Auto,
                "always" => ColorWhen::Always,
                "never" => ColorWhen::Never,
                _ => return Err(format!("invalid color '{when}'")),
            };
            continue;
        }
        if param == "--palette" {
            let Some(palette) = opts.next() else {
                return Err("option '--palette' requires an argument".to_string());
            };
            params.palette = Some(palette.to_string_lossy().into_owned());
            continue;
        }
        if let Some(palette) = param.to_str().and_then(|p| p.strip_prefix("--palette=")) {
            params.palette = Some(palette.to_string());
            continue;
        }
        if param == "---presume-output-tty" {
            params.presume_output_tty = true;
            continue;
        }
        if param == "-p" || param == "--show-c-function" {
            show_c_function = true;
            push_function_line_pattern(&mut params, C_FUNCTION_PATTERN)?;
            continue;
        }
        if let Some(argument) = option_argument(&param, "-F", "--show-function-line") {
            let pattern = argument_value(argument, &param, &mut opts)?;
            push_function_line_pattern(&mut params, &pattern.to_string_lossy())?;
            continue;
        }
        if param == "--ignore-file-name-case" {
            params.ignore_file_name_case = true;
            continue;
        }
        if param == "--no-ignore-file-name-case" {
            params.ignore_file_name_case = false;
            continue;
        }
        if let Some(argument) = option_argument(&param, "-S", "--starting-file") {
            params.starting_file = Some(argument_value(argument, &param, &mut opts)?);
            continue;
        }
        if param == "-r" || param == "--recursive" {
            params.recursive = true;
            continue;
        }
        if param == "-a" || param == "--text" {
            params.text = true;
            continue;
        }
        if param == "-t" || param == "--expand-tabs" {
            params.expand_tabs = true;
            continue;
        }
        if param == "--normal" {
            if format.is_some() && format != Some(Format::Normal) {
                return Err("Conflicting output style options".to_string());
            }
            format = Some(Format::Normal);
            continue;
        }
        if param == "-e" || param == "--ed" {
            if format.is_some() && format != Some(Format::Ed) {
                return Err("Conflicting output style options".to_string());
            }
            format = Some(Format::Ed);
            continue;
        }
        if param == "-y" || param == "--side-by-side" {
            if format.is_some() && format != Some(Format::SideBySide) {
                return Err("Conflicting output style option".to_string());
            }
            format = Some(Format::SideBySide);
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
        match match_context_diff_params(&param, next_param, format) {
            Ok(DiffStyleMatch {
                is_match,
                context_count,
                next_param_consumed,
            }) => {
                if is_match {
                    format = Some(Format::Context);
                    if context_count.is_some() {
                        context = context_count;
                    }
                    if next_param_consumed {
                        opts.next();
                    }
                    continue;
                }
            }
            Err(error) => return Err(error),
        }
        match match_unified_diff_params(&param, next_param, format) {
            Ok(DiffStyleMatch {
                is_match,
                context_count,
                next_param_consumed,
            }) => {
                if is_match {
                    format = Some(Format::Unified);
                    if context_count.is_some() {
                        context = context_count;
                    }
                    if next_param_consumed {
                        opts.next();
                    }
                    continue;
                }
            }
            Err(error) => return Err(error),
        }
        if param.to_string_lossy().starts_with('-') {
            return Err(format!("unrecognized option '{}'", param.to_string_lossy()));
        }
        operand_positions.push(taken.get() - 1 - usize::from(opts.peek().is_some()));
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
        operand_positions.push(taken.get() - 1);
        param
    } else {
        return Err(format!(
            "Usage: {} <from> <to>",
            params.executable.to_string_lossy()
        ));
    };
    params.to = if let Some(to) = to {
        to
    } else if let Some(param) = opts.next() {
        operand_positions.push(taken.get() - 1);
        param
    } else {
        return Err(format!(
            "Usage: {} <from> <to>",
            params.executable.to_string_lossy()
        ));
    };

    // diff DIRECTORY FILE => diff DIRECTORY/FILE FILE
    // diff FILE DIRECTORY => diff FILE DIRECTORY/FILE
    let mut from_path: PathBuf = PathBuf::from(&params.from);
    let mut to_path: PathBuf = PathBuf::from(&params.to);

    if from_path.is_dir() && to_path.is_file() {
        let name = directory_entry(&from_path, to_path.file_name().unwrap(), &params);
        from_path.push(name);
        params.from = from_path.into_os_string();
    } else if from_path.is_file() && to_path.is_dir() {
        let name = directory_entry(&to_path, from_path.file_name().unwrap(), &params);
        to_path.push(name);
        params.to = to_path.into_os_string();
    }

    params.format = match format {
        Some(format) => format,
        None if show_c_function => Format::Context,
        None => Format::default(),
    };
    if let Some(context_count) = context {
        params.context_count = context_count;
    }
    drop(opts);
    let options = args
        .into_iter()
        .enumerate()
        .skip(1)
        .filter(|(position, _)| !operand_positions.contains(position))
        .map(|(_, arg)| arg)
        .collect();
    Ok((params, options))
}

/// Returns the name of the entry of `dir` to compare with a file named
/// `name`: `name` itself, or with --ignore-file-name-case, an entry whose name
/// differs only by case when `name` does not exist.
fn directory_entry(dir: &Path, name: &OsStr, params: &Params) -> OsString {
    if params.ignore_file_name_case && !dir.join(name).exists() {
        let wanted = fold_file_name_case(name);
        let entries = std::fs::read_dir(dir).into_iter().flatten().flatten();
        if let Some(entry) = entries
            .map(|entry| entry.file_name())
            .find(|entry| fold_file_name_case(entry) == wanted)
        {
            return entry;
        }
    }
    name.to_os_string()
}

/// How the argument of an option is given.
enum OptionArgument<'a> {
    /// In the same command line argument, as in `-Fx` or `--show-function-line=x`.
    Attached(&'a str),
    /// As the next command line argument.
    Separate,
}

/// Returns how the argument of the option named `short` (like `-F`) or
/// `long` is given, if `param` is that option.
fn option_argument<'a>(param: &'a OsStr, short: &str, long: &str) -> Option<OptionArgument<'a>> {
    if param == short || param == long {
        return Some(OptionArgument::Separate);
    }
    let param = param.to_str()?;
    param
        .strip_prefix(long)
        .and_then(|rest| rest.strip_prefix('='))
        .or_else(|| param.strip_prefix(short).filter(|value| !value.is_empty()))
        .map(OptionArgument::Attached)
}

/// Returns the value of the argument of the option `param`, taking the next
/// command line argument when it is not attached.
fn argument_value(
    argument: OptionArgument,
    param: &OsStr,
    opts: &mut impl Iterator<Item = OsString>,
) -> Result<OsString, String> {
    match argument {
        OptionArgument::Attached(value) => Ok(value.into()),
        OptionArgument::Separate => opts
            .next()
            .ok_or_else(|| format!("option '{}' requires an argument", param.to_string_lossy())),
    }
}

/// Validates and records a `-F` pattern.
fn push_function_line_pattern(params: &mut Params, pattern: &str) -> Result<(), String> {
    let bre = Bre::new(pattern).map_err(|error| format!("'{pattern}': {error}"))?;
    params.function_line_patterns.push(bre);
    Ok(())
}

struct DiffStyleMatch {
    is_match: bool,
    context_count: Option<usize>,
    next_param_consumed: bool,
}

fn match_context_diff_params(
    param: &OsString,
    next_param: Option<&OsString>,
    format: Option<Format>,
) -> Result<DiffStyleMatch, String> {
    const CONTEXT_RE: &str = r"^(-[cC](?<num1>\d*)|--context(=(?<num2>\d*))?|-(?<num3>\d+)c)$";
    let regex = Regex::new(CONTEXT_RE).unwrap();
    let is_match = regex.is_match(param.to_string_lossy().as_ref());
    let mut context_count = None;
    let mut next_param_consumed = false;
    if is_match {
        if format.is_some() && format != Some(Format::Context) {
            return Err("Conflicting output style options".to_string());
        }
        let captures = regex.captures(param.to_str().unwrap()).unwrap();
        let num = captures
            .name("num1")
            .or(captures.name("num2"))
            .or(captures.name("num3"));
        if let Some(numvalue) = num {
            if !numvalue.as_str().is_empty() {
                context_count = Some(numvalue.as_str().parse::<usize>().unwrap());
            }
        }
        if param == "-C" {
            if let Some(p) = next_param {
                let size_str = p.to_string_lossy();
                match size_str.parse::<usize>() {
                    Ok(context_size) => {
                        context_count = Some(context_size);
                        next_param_consumed = true;
                    }
                    Err(_) => return Err(format!("invalid context length '{size_str}'")),
                }
            }
        }
    }
    Ok(DiffStyleMatch {
        is_match,
        context_count,
        next_param_consumed,
    })
}

fn match_unified_diff_params(
    param: &OsString,
    next_param: Option<&OsString>,
    format: Option<Format>,
) -> Result<DiffStyleMatch, String> {
    const UNIFIED_RE: &str = r"^(-[uU](?<num1>\d*)|--unified(=(?<num2>\d*))?|-(?<num3>\d+)u)$";
    let regex = Regex::new(UNIFIED_RE).unwrap();
    let is_match = regex.is_match(param.to_string_lossy().as_ref());
    let mut context_count = None;
    let mut next_param_consumed = false;
    if is_match {
        if format.is_some() && format != Some(Format::Unified) {
            return Err("Conflicting output style options".to_string());
        }
        let captures = regex.captures(param.to_str().unwrap()).unwrap();
        let num = captures
            .name("num1")
            .or(captures.name("num2"))
            .or(captures.name("num3"));
        if let Some(numvalue) = num {
            if !numvalue.as_str().is_empty() {
                context_count = Some(numvalue.as_str().parse::<usize>().unwrap());
            }
        }
        if param == "-U" {
            if let Some(p) = next_param {
                let size_str = p.to_string_lossy();
                match size_str.parse::<usize>() {
                    Ok(context_size) => {
                        context_count = Some(context_size);
                        next_param_consumed = true;
                    }
                    Err(_) => return Err(format!("invalid context length '{size_str}'")),
                }
            }
        }
    }
    Ok(DiffStyleMatch {
        is_match,
        context_count,
        next_param_consumed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_params<I: Iterator<Item = OsString>>(opts: Peekable<I>) -> Result<Params, String> {
        super::parse_params(opts).map(|(params, _)| params)
    }
    fn os(s: &str) -> OsString {
        OsString::from(s)
    }
    #[test]
    fn basics() {
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("--normal"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
    }
    #[test]
    fn basics_ed() {
        for arg in ["-e", "--ed"] {
            assert_eq!(
                Ok(Params {
                    executable: os("diff"),
                    from: os("foo"),
                    to: os("bar"),
                    format: Format::Ed,
                    ..Default::default()
                }),
                parse_params(
                    [os("diff"), os(arg), os("foo"), os("bar")]
                        .iter()
                        .cloned()
                        .peekable()
                )
            );
        }
    }
    #[test]
    fn context_valid() {
        for args in [vec!["-c"], vec!["--context"], vec!["--context="]] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert_eq!(
                Ok(Params {
                    executable: os("diff"),
                    from: os("foo"),
                    to: os("bar"),
                    format: Format::Context,
                    ..Default::default()
                }),
                parse_params(params.iter().map(|x| os(x)).peekable())
            );
        }
        for args in [
            vec!["-c42"],
            vec!["-C42"],
            vec!["-C", "42"],
            vec!["--context=42"],
            vec!["-42c"],
        ] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert_eq!(
                Ok(Params {
                    executable: os("diff"),
                    from: os("foo"),
                    to: os("bar"),
                    format: Format::Context,
                    context_count: 42,
                    ..Default::default()
                }),
                parse_params(params.iter().map(|x| os(x)).peekable())
            );
        }
    }
    #[test]
    fn context_invalid() {
        for args in [
            vec!["-c", "42"],
            vec!["-c=42"],
            vec!["-c="],
            vec!["-C"],
            vec!["-C=42"],
            vec!["-C="],
            vec!["--context42"],
            vec!["--context", "42"],
            vec!["-42C"],
        ] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert!(parse_params(params.iter().map(|x| os(x)).peekable()).is_err());
        }
    }
    #[test]
    fn unified_valid() {
        for args in [vec!["-u"], vec!["--unified"], vec!["--unified="]] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert_eq!(
                Ok(Params {
                    executable: os("diff"),
                    from: os("foo"),
                    to: os("bar"),
                    format: Format::Unified,
                    ..Default::default()
                }),
                parse_params(params.iter().map(|x| os(x)).peekable())
            );
        }
        for args in [
            vec!["-u42"],
            vec!["-U42"],
            vec!["-U", "42"],
            vec!["--unified=42"],
            vec!["-42u"],
        ] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert_eq!(
                Ok(Params {
                    executable: os("diff"),
                    from: os("foo"),
                    to: os("bar"),
                    format: Format::Unified,
                    context_count: 42,
                    ..Default::default()
                }),
                parse_params(params.iter().map(|x| os(x)).peekable())
            );
        }
    }
    #[test]
    fn unified_invalid() {
        for args in [
            vec!["-u", "42"],
            vec!["-u=42"],
            vec!["-u="],
            vec!["-U"],
            vec!["-U=42"],
            vec!["-U="],
            vec!["--unified42"],
            vec!["--unified", "42"],
            vec!["-42U"],
        ] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert!(parse_params(params.iter().map(|x| os(x)).peekable()).is_err());
        }
    }
    #[test]
    fn context_count() {
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                format: Format::Unified,
                context_count: 54,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("-u54"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                format: Format::Unified,
                context_count: 54,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("-U54"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                format: Format::Unified,
                context_count: 54,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("-U"), os("54"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                format: Format::Context,
                context_count: 54,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("-c54"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
    }
    #[test]
    fn report_identical_files() {
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                report_identical_files: true,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("-s"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                report_identical_files: true,
                ..Default::default()
            }),
            parse_params(
                [
                    os("diff"),
                    os("--report-identical-files"),
                    os("foo"),
                    os("bar"),
                ]
                .iter()
                .cloned()
                .peekable()
            )
        );
    }
    #[test]
    fn brief() {
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                brief: true,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("-q"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                brief: true,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("--brief"), os("foo"), os("bar"),]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
    }
    #[test]
    fn expand_tabs() {
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        for option in ["-t", "--expand-tabs"] {
            assert_eq!(
                Ok(Params {
                    executable: os("diff"),
                    from: os("foo"),
                    to: os("bar"),
                    expand_tabs: true,
                    ..Default::default()
                }),
                parse_params(
                    [os("diff"), os(option), os("foo"), os("bar")]
                        .iter()
                        .cloned()
                        .peekable()
                )
            );
        }
    }
    #[test]
    fn tabsize() {
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                tabsize: 1,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("--tabsize=1"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                tabsize: 42,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("--tabsize=42"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
        assert!(parse_params(
            [os("diff"), os("--tabsize"), os("foo"), os("bar")]
                .iter()
                .cloned()
                .peekable()
        )
        .is_err());
        assert!(parse_params(
            [os("diff"), os("--tabsize="), os("foo"), os("bar")]
                .iter()
                .cloned()
                .peekable()
        )
        .is_err());
        assert!(parse_params(
            [os("diff"), os("--tabsize=r2"), os("foo"), os("bar")]
                .iter()
                .cloned()
                .peekable()
        )
        .is_err());
        assert!(parse_params(
            [os("diff"), os("--tabsize=-1"), os("foo"), os("bar")]
                .iter()
                .cloned()
                .peekable()
        )
        .is_err());
        assert!(parse_params(
            [os("diff"), os("--tabsize=r2"), os("foo"), os("bar")]
                .iter()
                .cloned()
                .peekable()
        )
        .is_err());
        assert!(parse_params(
            [
                os("diff"),
                os("--tabsize=92233720368547758088"),
                os("foo"),
                os("bar")
            ]
            .iter()
            .cloned()
            .peekable()
        )
        .is_err());
    }
    #[test]
    fn width() {
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("bar"),
                width: 100,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("--width=100"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
    }
    #[test]
    fn width_suffix_is_an_operand() {
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("xyz--width=5"),
                to: os("foo"),
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("xyz--width=5"), os("foo")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
    }
    #[cfg(unix)]
    #[test]
    fn width_non_utf8_is_not_an_option() {
        use std::os::unix::ffi::OsStringExt;
        // used to panic in into_string().unwrap()
        let bad = OsString::from_vec(b"\xff--width=5".to_vec());
        let params = parse_params(
            [os("diff"), bad.clone(), os("foo")]
                .iter()
                .cloned()
                .peekable(),
        )
        .unwrap();
        assert_eq!(params.from, bad);
    }
    #[test]
    fn double_dash() {
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("-g"),
                to: os("-h"),
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("--"), os("-g"), os("-h")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
    }
    #[test]
    fn default_to_stdin() {
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("foo"),
                to: os("-"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("foo"), os("-")].iter().cloned().peekable())
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("-"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("-"), os("bar")].iter().cloned().peekable())
        );
        assert_eq!(
            Ok(Params {
                executable: os("diff"),
                from: os("-"),
                to: os("-"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("-"), os("-")].iter().cloned().peekable())
        );
        assert!(parse_params(
            [os("diff"), os("foo"), os("bar"), os("-")]
                .iter()
                .cloned()
                .peekable()
        )
        .is_err());
        assert!(parse_params(
            [os("diff"), os("-"), os("-"), os("-")]
                .iter()
                .cloned()
                .peekable()
        )
        .is_err());
    }
    #[test]
    fn missing_arguments() {
        assert!(parse_params([os("diff")].iter().cloned().peekable()).is_err());
        assert!(parse_params([os("diff"), os("foo")].iter().cloned().peekable()).is_err());
    }
    #[test]
    fn unknown_argument() {
        assert!(parse_params(
            [os("diff"), os("-g"), os("foo"), os("bar")]
                .iter()
                .cloned()
                .peekable()
        )
        .is_err());
        assert!(
            parse_params([os("diff"), os("-g"), os("bar")].iter().cloned().peekable()).is_err()
        );
        assert!(parse_params([os("diff"), os("-g")].iter().cloned().peekable()).is_err());
    }
    #[test]
    fn empty() {
        assert!(parse_params([].iter().cloned().peekable()).is_err());
    }
    #[test]
    fn conflicting_output_styles() {
        for (arg1, arg2) in [
            ("-u", "-c"),
            ("-u", "-e"),
            ("-c", "-u"),
            ("-c", "-U42"),
            ("-u", "--normal"),
            ("--normal", "-e"),
            ("--context", "--normal"),
        ] {
            assert!(parse_params(
                [os("diff"), os(arg1), os(arg2), os("foo"), os("bar")]
                    .iter()
                    .cloned()
                    .peekable()
            )
            .is_err());
        }
    }

    #[test]
    fn color_options() {
        let parse = |args: &[&str]| {
            let args = std::iter::once("diff")
                .chain(args.iter().copied())
                .chain(["foo", "bar"]);
            parse_params(args.map(os).peekable())
        };
        assert_eq!(parse(&["--color"]).unwrap().color, ColorWhen::Auto);
        assert_eq!(parse(&["--color=never"]).unwrap().color, ColorWhen::Never);
        let params = parse(&["--palette", "ad=35", "--color=always"]).unwrap();
        assert_eq!(params.color, ColorWhen::Always);
        assert_eq!(params.palette.as_deref(), Some("ad=35"));
        assert_eq!(
            parse(&["--palette=hd=4"]).unwrap().palette.as_deref(),
            Some("hd=4")
        );
        for option in ["--colorful", "--color=", "--color=yes"] {
            assert!(parse(&[option]).is_err(), "{option}");
        }
    }

    #[test]
    fn function_lines() {
        let patterns = |patterns: &[&str]| -> Vec<Bre> {
            patterns
                .iter()
                .map(|pattern| Bre::new(pattern).unwrap())
                .collect()
        };
        let parse = |args: &[&str]| {
            let args = std::iter::once("diff")
                .chain(args.iter().copied())
                .chain(["foo", "bar"]);
            parse_params(args.map(os).peekable())
        };
        let params = parse(&["-p"]).unwrap();
        assert_eq!(params.format, Format::Context);
        assert_eq!(
            params.function_line_patterns,
            patterns(&[C_FUNCTION_PATTERN])
        );

        let params = parse(&["-u", "-p"]).unwrap();
        assert_eq!(params.format, Format::Unified);

        let params = parse(&["-F", "^fn", "-Fx", "--show-function-line=^#", "-e"]).unwrap();
        assert_eq!(params.format, Format::Ed);
        assert_eq!(params.function_line_patterns, patterns(&["^fn", "x", "^#"]));

        // -F alone selects no output format.
        assert_eq!(parse(&["-F", "^fn"]).unwrap().format, Format::Normal);

        assert_eq!(
            parse(&["-F", "oops\\"]),
            Err("'oops\\': Trailing backslash".to_string())
        );
    }

    #[test]
    fn option_arguments() {
        let options = |args: &[&str]| {
            let (_, options) =
                super::parse_params(args.iter().copied().map(os).peekable()).unwrap();
            options
        };
        assert_eq!(
            options(&["diff", "-u", "old", "-r", "new"]),
            [os("-u"), os("-r")]
        );
        // An option argument equal to an operand stays with its option.
        assert_eq!(
            options(&["diff", "-F", "new", "old", "new"]),
            [os("-F"), os("new")]
        );
        assert_eq!(
            options(&["diff", "-q", "--", "-old", "new"]),
            [os("-q"), os("--")]
        );
        assert_eq!(options(&["diff", "-", "new", "-t"]), [os("-t")]);
    }

    #[test]
    fn starting_file() {
        for args in [
            &["-S", "mid"][..],
            &["-Smid"],
            &["--starting-file=mid"],
            &["--starting-file", "mid"],
        ] {
            let args = std::iter::once("diff")
                .chain(args.iter().copied())
                .chain(["foo", "bar"]);
            let params = parse_params(args.map(os).peekable()).unwrap();
            assert_eq!(params.starting_file, Some(os("mid")));
            assert_eq!(params.from, os("foo"));
        }
        assert!(parse_params(
            [os("diff"), os("foo"), os("bar"), os("-S")]
                .into_iter()
                .peekable()
        )
        .is_err());
    }
}
