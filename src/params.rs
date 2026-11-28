use std::ffi::{OsStr, OsString};
use std::iter::Peekable;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::bre::Bre;
use crate::color::ColorWhen;
use crate::utils::fold_file_name_case;

/// Largest value accepted for `--tabsize`.
/// Side-by-side mode computes `tabsize + GUTTER_WIDTH_MIN` (3) as an
/// `isize` in `side_diff::Config::new`, so anything larger would overflow.
/// GNU diff accepts the same maximum.
pub(crate) const MAX_TABSIZE: usize = isize::MAX as usize - 3;

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
    pub ignore_tab_expansion: bool,
    pub suppress_blank_empty: bool,
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
    pub ignore_blank_lines: bool,
    pub no_dereference: bool,
    /// Set when `from` did not exist and was compared as an empty file.
    pub from_missing: bool,
    /// Set when `to` did not exist and was compared as an empty file.
    pub to_missing: bool,
    pub from_label: Option<OsString>,
    pub to_label: Option<OsString>,
}

impl Params {
    /// The names to show for the from-file and the to-file: their labels if
    /// given, else their paths.
    pub fn names(&self) -> (&OsString, &OsString) {
        (
            self.from_label.as_ref().unwrap_or(&self.from),
            self.to_label.as_ref().unwrap_or(&self.to),
        )
    }
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
            ignore_tab_expansion: false,
            suppress_blank_empty: false,
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
            ignore_blank_lines: false,
            no_dereference: false,
            from_missing: false,
            to_missing: false,
            from_label: None,
            to_label: None,
        }
    }
}

fn extra_operand(executable: &OsStr, param: &OsStr) -> String {
    let exe = executable.to_string_lossy();
    format!(
        "{exe}: extra operand '{}'\n{exe}: Try '{exe} --help' for more information.",
        param.to_string_lossy()
    )
}

/// Parses the command line, and also returns its arguments other than the
/// program name and the operands.
pub fn parse_params<I: Iterator<Item = OsString>>(
    opts: Peekable<I>,
) -> Result<(Params, Vec<OsString>), String> {
    let args: Vec<OsString> = opts.collect();
    // Each argument comes with its position, to tell the operands apart from
    // an option argument with the same value.
    let mut opts = args.iter().cloned().enumerate().peekable();
    let mut operand_positions = Vec::new();
    let Some((_, executable)) = opts.next() else {
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
    while let Some((position, param)) = opts.next() {
        // -L LABEL, -LLABEL, --label LABEL or --label=LABEL: the first names
        // the from-file in headers and messages, the second the to-file.
        // The label may be any bytes, so match on those, not on a str.
        let bytes = param.as_encoded_bytes();
        let label = if bytes == b"-L" || bytes == b"--label" {
            Some(opts.next().map(|(_, label)| label).ok_or_else(|| {
                format!("option '{}' requires an argument", param.to_string_lossy())
            })?)
        } else {
            bytes
                .strip_prefix(b"--label=")
                .or_else(|| bytes.strip_prefix(b"-L"))
                // SAFETY: the bytes come from an OsStr and are split right
                // after an ASCII prefix.
                .map(|rest| unsafe { OsStr::from_encoded_bytes_unchecked(rest) }.to_os_string())
        };
        if let Some(label) = label {
            if params.from_label.is_none() {
                params.from_label = Some(label);
            } else if params.to_label.is_none() {
                params.to_label = Some(label);
            } else {
                return Err("too many file label options".to_string());
            }
            continue;
        }
        let next_param = opts.peek().map(|(_, next_param)| next_param);
        if param == "--" {
            break;
        }
        if param == "-" {
            operand_positions.push(position);
            if from.is_none() {
                from = Some(param);
            } else if to.is_none() {
                to = Some(param);
            } else {
                return Err(extra_operand(&params.executable, &param));
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
            let Some((_, palette)) = opts.next() else {
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
        if let Some(argument) = option_argument(&param, Some("-F"), "--show-function-line") {
            let pattern = argument_value(argument, &param, &mut opts)?;
            push_function_line_pattern(&mut params, &pattern.to_string_lossy())?;
            continue;
        }
        if param == "--no-dereference" {
            params.no_dereference = true;
            continue;
        }
        if param == "--ignore-file-name-case" {
            params.ignore_file_name_case = true;
            continue;
        }
        if param == "-B" || param == "--ignore-blank-lines" {
            params.ignore_blank_lines = true;
            continue;
        }
        if param == "--no-ignore-file-name-case" {
            params.ignore_file_name_case = false;
            continue;
        }
        if let Some(argument) = option_argument(&param, Some("-S"), "--starting-file") {
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
        if param == "-E" || param == "--ignore-tab-expansion" {
            params.ignore_tab_expansion = true;
            continue;
        }
        if param == "--suppress-blank-empty" {
            params.suppress_blank_empty = true;
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
        if let Some(argument) = option_argument(&param, Some("-W"), "--width") {
            let width_value = argument_value(argument, &param, &mut opts)?;
            let width_str = width_value.to_string_lossy();
            params.width = match width_str.parse::<usize>() {
                Ok(num) if num > 0 => num,
                _ => return Err(format!("invalid width «{width_str}»")),
            };
            continue;
        }
        if let Some(argument) = option_argument(&param, None, "--tabsize") {
            let tabsize_value = argument_value(argument, &param, &mut opts)?;
            let tabsize_str = tabsize_value.to_string_lossy();
            params.tabsize = match tabsize_str.parse::<usize>() {
                Ok(num) if num > 0 && num <= MAX_TABSIZE => num,
                _ => return Err(format!("invalid tabsize «{tabsize_str}»")),
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
                    context = Some(largest_context(context, context_count));
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
                    context = Some(largest_context(context, context_count));
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
        operand_positions.push(position);
        if from.is_none() {
            from = Some(param);
        } else if to.is_none() {
            to = Some(param);
        } else {
            return Err(extra_operand(&params.executable, &param));
        }
    }
    params.from = if let Some(from) = from {
        from
    } else if let Some((position, param)) = opts.next() {
        operand_positions.push(position);
        param
    } else {
        return Err(format!(
            "Usage: {} <from> <to>",
            params.executable.to_string_lossy()
        ));
    };
    params.to = if let Some(to) = to {
        to
    } else if let Some((position, param)) = opts.next() {
        operand_positions.push(position);
        param
    } else {
        return Err(format!(
            "Usage: {} <from> <to>",
            params.executable.to_string_lossy()
        ));
    };
    // Operands after `--` are not checked by the option loop above.
    if let Some((_, param)) = opts.next() {
        return Err(extra_operand(&params.executable, &param));
    }

    // diff DIRECTORY FILE => diff DIRECTORY/FILE FILE
    // diff FILE DIRECTORY => diff FILE DIRECTORY/FILE
    let mut from_path: PathBuf = PathBuf::from(&params.from);
    let mut to_path: PathBuf = PathBuf::from(&params.to);

    // Symbolic links that are not followed are compared like files.
    let is_file = |path: &Path| {
        if params.no_dereference {
            path.symlink_metadata()
                .is_ok_and(|metadata| !metadata.is_dir())
        } else {
            path.is_file()
        }
    };
    if from_path.is_dir() && is_file(&to_path) {
        let name = directory_entry(&from_path, to_path.file_name().unwrap(), &params);
        from_path.push(name);
        params.from = from_path.into_os_string();
    } else if is_file(&from_path) && to_path.is_dir() {
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
fn option_argument<'a>(
    param: &'a OsStr,
    short: Option<&str>,
    long: &str,
) -> Option<OptionArgument<'a>> {
    if short.is_some_and(|s| param == s) || param == long {
        return Some(OptionArgument::Separate);
    }
    let param = param.to_str()?;
    if let Some(rest) = param.strip_prefix(long).and_then(|r| r.strip_prefix('=')) {
        return Some(OptionArgument::Attached(rest));
    }
    if let Some(short) = short {
        if let Some(rest) = param.strip_prefix(short).filter(|value| !value.is_empty()) {
            return Some(OptionArgument::Attached(rest));
        }
    }
    None
}

/// Returns the value of the argument of the option `param`, taking the next
/// command line argument when it is not attached.
fn argument_value(
    argument: OptionArgument,
    param: &OsStr,
    opts: &mut impl Iterator<Item = (usize, OsString)>,
) -> Result<OsString, String> {
    match argument {
        OptionArgument::Attached(value) => Ok(value.into()),
        OptionArgument::Separate => opts
            .next()
            .map(|(_, value)| value)
            .ok_or_else(|| format!("option '{}' requires an argument", param.to_string_lossy())),
    }
}

/// Validates and records a `-F` pattern.
fn push_function_line_pattern(params: &mut Params, pattern: &str) -> Result<(), String> {
    let bre = Bre::new(pattern).map_err(|error| format!("'{pattern}': {error}"))?;
    params.function_line_patterns.push(bre);
    Ok(())
}

/// GNU diff uses the largest context length given, and a bare -c or -u
/// counts as a request for the default length.
fn largest_context(current: Option<usize>, given: Option<usize>) -> usize {
    let given = given.unwrap_or_else(|| Params::default().context_count);
    current.map_or(given, |current| current.max(given))
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
    use rstest::rstest;

    fn parse_params<I: Iterator<Item = OsString>>(opts: Peekable<I>) -> Result<Params, String> {
        super::parse_params(opts).map(|(params, _)| params)
    }
    fn os(s: &str) -> OsString {
        OsString::from(s)
    }
    fn parse(args: &[&str]) -> Result<Params, String> {
        parse_params(
            std::iter::once("diff")
                .chain(args.iter().copied())
                .map(os)
                .peekable(),
        )
    }
    fn operands(from: &str, to: &str) -> Params {
        Params {
            executable: os("diff"),
            from: os(from),
            to: os(to),
            ..Default::default()
        }
    }
    fn foo_bar() -> Params {
        operands("foo", "bar")
    }

    #[rstest]
    #[case::basics(&["foo", "bar"], foo_bar())]
    #[case::normal(&["--normal", "foo", "bar"], foo_bar())]
    #[case::ed_short(&["-e", "foo", "bar"], Params { format: Format::Ed, ..foo_bar() })]
    #[case::ed_long(&["--ed", "foo", "bar"], Params { format: Format::Ed, ..foo_bar() })]
    #[case::context_short(&["-c", "foo", "bar"], Params { format: Format::Context, ..foo_bar() })]
    #[case::context_long(&["--context", "foo", "bar"], Params { format: Format::Context, ..foo_bar() })]
    #[case::context_long_empty(&["--context=", "foo", "bar"], Params { format: Format::Context, ..foo_bar() })]
    #[case::context_attached(
        &["-c42", "foo", "bar"],
        Params { format: Format::Context, context_count: 42, ..foo_bar() },
    )]
    #[case::context_upper_attached(
        &["-C42", "foo", "bar"],
        Params { format: Format::Context, context_count: 42, ..foo_bar() },
    )]
    #[case::context_upper_separate(
        &["-C", "42", "foo", "bar"],
        Params { format: Format::Context, context_count: 42, ..foo_bar() },
    )]
    #[case::context_long_count(
        &["--context=42", "foo", "bar"],
        Params { format: Format::Context, context_count: 42, ..foo_bar() },
    )]
    #[case::context_count_prefix(
        &["-42c", "foo", "bar"],
        Params { format: Format::Context, context_count: 42, ..foo_bar() },
    )]
    #[case::context_count_54(
        &["-c54", "foo", "bar"],
        Params { format: Format::Context, context_count: 54, ..foo_bar() },
    )]
    #[case::unified_short(&["-u", "foo", "bar"], Params { format: Format::Unified, ..foo_bar() })]
    #[case::unified_long(&["--unified", "foo", "bar"], Params { format: Format::Unified, ..foo_bar() })]
    #[case::unified_long_empty(&["--unified=", "foo", "bar"], Params { format: Format::Unified, ..foo_bar() })]
    #[case::unified_attached(
        &["-u42", "foo", "bar"],
        Params { format: Format::Unified, context_count: 42, ..foo_bar() },
    )]
    #[case::unified_upper_attached(
        &["-U42", "foo", "bar"],
        Params { format: Format::Unified, context_count: 42, ..foo_bar() },
    )]
    #[case::unified_upper_separate(
        &["-U", "42", "foo", "bar"],
        Params { format: Format::Unified, context_count: 42, ..foo_bar() },
    )]
    #[case::unified_long_count(
        &["--unified=42", "foo", "bar"],
        Params { format: Format::Unified, context_count: 42, ..foo_bar() },
    )]
    #[case::unified_count_prefix(
        &["-42u", "foo", "bar"],
        Params { format: Format::Unified, context_count: 42, ..foo_bar() },
    )]
    #[case::unified_count_54(
        &["-u54", "foo", "bar"],
        Params { format: Format::Unified, context_count: 54, ..foo_bar() },
    )]
    #[case::unified_upper_count_54(
        &["-U54", "foo", "bar"],
        Params { format: Format::Unified, context_count: 54, ..foo_bar() },
    )]
    #[case::unified_upper_separate_count_54(
        &["-U", "54", "foo", "bar"],
        Params { format: Format::Unified, context_count: 54, ..foo_bar() },
    )]
    #[case::report_identical_files_short(
        &["-s", "foo", "bar"],
        Params { report_identical_files: true, ..foo_bar() },
    )]
    #[case::report_identical_files_long(
        &["--report-identical-files", "foo", "bar"],
        Params { report_identical_files: true, ..foo_bar() },
    )]
    #[case::brief_short(&["-q", "foo", "bar"], Params { brief: true, ..foo_bar() })]
    #[case::brief_long(&["--brief", "foo", "bar"], Params { brief: true, ..foo_bar() })]
    #[case::text_short(&["-a", "old.bin", "new.bin"], Params { text: true, ..operands("old.bin", "new.bin") })]
    #[case::text_long(&["--text", "old.bin", "new.bin"], Params { text: true, ..operands("old.bin", "new.bin") })]
    #[case::expand_tabs_short(&["-t", "foo", "bar"], Params { expand_tabs: true, ..foo_bar() })]
    #[case::expand_tabs_long(&["--expand-tabs", "foo", "bar"], Params { expand_tabs: true, ..foo_bar() })]
    #[case::ignore_tab_expansion_short(&["-E", "foo", "bar"], Params { ignore_tab_expansion: true, ..foo_bar() })]
    #[case::ignore_tab_expansion_long(
        &["--ignore-tab-expansion", "foo", "bar"],
        Params { ignore_tab_expansion: true, ..foo_bar() },
    )]
    #[case::tabsize_separate(&["--tabsize", "42", "foo", "bar"], Params { tabsize: 42, ..foo_bar() })]
    #[case::tabsize_equals(&["--tabsize=42", "foo", "bar"], Params { tabsize: 42, ..foo_bar() })]
    #[case::width_short_separate(&["-W", "100", "foo", "bar"], Params { width: 100, ..foo_bar() })]
    #[case::width_short_attached(&["-W100", "foo", "bar"], Params { width: 100, ..foo_bar() })]
    #[case::width_long_separate(&["--width", "100", "foo", "bar"], Params { width: 100, ..foo_bar() })]
    #[case::width_long_equals(&["--width=100", "foo", "bar"], Params { width: 100, ..foo_bar() })]
    #[case::width_suffix_is_an_operand(&["xyz--width=5", "foo"], operands("xyz--width=5", "foo"))]
    #[case::labels(
        &["--label", "old", "-Lnew", "foo", "bar"],
        Params { from_label: Some(os("old")), to_label: Some(os("new")), ..foo_bar() },
    )]
    #[case::double_dash(&["--", "-g", "-h"], operands("-g", "-h"))]
    #[case::stdin_to(&["foo", "-"], operands("foo", "-"))]
    #[case::stdin_from(&["-", "bar"], operands("-", "bar"))]
    #[case::stdin_both(&["-", "-"], operands("-", "-"))]
    fn valid(#[case] args: &[&str], #[case] expected: Params) {
        assert_eq!(parse(args), Ok(expected));
    }

    #[rstest]
    #[case::context_separate_count(&["-c", "42", "foo", "bar"])]
    #[case::context_equals_count(&["-c=42", "foo", "bar"])]
    #[case::context_equals_empty(&["-c=", "foo", "bar"])]
    #[case::context_upper_no_count(&["-C", "foo", "bar"])]
    #[case::context_upper_equals_count(&["-C=42", "foo", "bar"])]
    #[case::context_upper_equals_empty(&["-C=", "foo", "bar"])]
    #[case::context_long_attached(&["--context42", "foo", "bar"])]
    #[case::context_long_separate(&["--context", "42", "foo", "bar"])]
    #[case::context_upper_count_prefix(&["-42C", "foo", "bar"])]
    #[case::unified_separate_count(&["-u", "42", "foo", "bar"])]
    #[case::unified_equals_count(&["-u=42", "foo", "bar"])]
    #[case::unified_equals_empty(&["-u=", "foo", "bar"])]
    #[case::unified_upper_no_count(&["-U", "foo", "bar"])]
    #[case::unified_upper_equals_count(&["-U=42", "foo", "bar"])]
    #[case::unified_upper_equals_empty(&["-U=", "foo", "bar"])]
    #[case::unified_long_attached(&["--unified42", "foo", "bar"])]
    #[case::unified_long_separate(&["--unified", "42", "foo", "bar"])]
    #[case::unified_upper_count_prefix(&["-42U", "foo", "bar"])]
    #[case::tabsize_takes_operand(&["--tabsize", "foo", "bar"])]
    #[case::tabsize_empty(&["--tabsize=", "foo", "bar"])]
    #[case::tabsize_not_a_number(&["--tabsize=r2", "foo", "bar"])]
    #[case::tabsize_negative(&["--tabsize=-1", "foo", "bar"])]
    #[case::tabsize_overflow(&["--tabsize=92233720368547758088", "foo", "bar"])]
    #[case::extra_stdin_operand(&["foo", "bar", "-"])]
    #[case::three_stdin_operands(&["-", "-", "-"])]
    #[case::no_operand(&[])]
    #[case::one_operand(&["foo"])]
    #[case::unknown_option(&["-g", "foo", "bar"])]
    #[case::unknown_option_one_operand(&["-g", "bar"])]
    #[case::unknown_option_alone(&["-g"])]
    #[case::conflicting_unified_context(&["-u", "-c", "foo", "bar"])]
    #[case::conflicting_unified_ed(&["-u", "-e", "foo", "bar"])]
    #[case::conflicting_context_unified(&["-c", "-u", "foo", "bar"])]
    #[case::conflicting_context_unified_count(&["-c", "-U42", "foo", "bar"])]
    #[case::conflicting_unified_normal(&["-u", "--normal", "foo", "bar"])]
    #[case::conflicting_normal_ed(&["--normal", "-e", "foo", "bar"])]
    #[case::conflicting_context_normal(&["--context", "--normal", "foo", "bar"])]
    #[case::color_unknown_option(&["--colorful", "foo", "bar"])]
    #[case::color_empty(&["--color=", "foo", "bar"])]
    #[case::color_invalid(&["--color=yes", "foo", "bar"])]
    #[case::starting_file_no_argument(&["foo", "bar", "-S"])]
    fn invalid(#[case] args: &[&str]) {
        assert!(parse(args).is_err());
    }

    #[test]
    fn empty() {
        assert!(parse_params([].iter().cloned().peekable()).is_err());
    }

    #[rstest]
    #[case::tabsize_no_argument(&["foo", "bar", "--tabsize"], "option '--tabsize' requires an argument")]
    #[case::width_short_no_argument(&["foo", "bar", "-W"], "option '-W' requires an argument")]
    #[case::width_long_no_argument(&["foo", "bar", "--width"], "option '--width' requires an argument")]
    #[case::width_short_separate_zero(&["foo", "bar", "-W", "0"], "invalid width «0»")]
    #[case::width_short_attached_zero(&["foo", "bar", "-W0"], "invalid width «0»")]
    #[case::width_long_equals_zero(&["foo", "bar", "--width=0"], "invalid width «0»")]
    #[case::width_long_separate_zero(&["foo", "bar", "--width", "0"], "invalid width «0»")]
    #[case::width_short_not_a_number(&["foo", "bar", "-W", "abc"], "invalid width «abc»")]
    #[case::width_long_not_a_number(&["foo", "bar", "--width=abc"], "invalid width «abc»")]
    #[case::too_many_labels(
        &["--label=a", "-L", "b", "-Lc", "foo", "bar"],
        "too many file label options",
    )]
    #[case::extra_operand(
        &["foo", "bar", "baz"],
        "diff: extra operand 'baz'\ndiff: Try 'diff --help' for more information.",
    )]
    #[case::extra_stdin_operand(
        &["-", "-", "-"],
        "diff: extra operand '-'\ndiff: Try 'diff --help' for more information.",
    )]
    #[case::extra_operand_after_double_dash(
        &["--", "foo", "bar", "baz"],
        "diff: extra operand 'baz'\ndiff: Try 'diff --help' for more information.",
    )]
    #[case::extra_operand_around_double_dash(
        &["foo", "--", "bar", "baz"],
        "diff: extra operand 'baz'\ndiff: Try 'diff --help' for more information.",
    )]
    #[case::function_line_trailing_backslash(&["-F", "oops\\", "foo", "bar"], "'oops\\': Trailing backslash")]
    fn error_message(#[case] args: &[&str], #[case] expected: &str) {
        assert_eq!(parse(args), Err(expected.to_string()));
    }

    #[rstest]
    #[case::max_plus_one(MAX_TABSIZE + 1)]
    #[case::usize_max(usize::MAX)]
    fn tabsize_too_large(#[case] tabsize: usize) {
        assert_eq!(
            Err(format!("invalid tabsize «{tabsize}»")),
            parse(&[&format!("--tabsize={tabsize}"), "foo", "bar"])
        );
    }

    #[test]
    fn tabsize_max() {
        assert_eq!(
            Ok(Params {
                tabsize: MAX_TABSIZE,
                ..foo_bar()
            }),
            parse(&[&format!("--tabsize={MAX_TABSIZE}"), "foo", "bar"])
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
    #[cfg(unix)]
    #[test]
    fn labels_non_utf8() {
        use std::os::unix::ffi::OsStringExt;
        let bytes = |b: &[u8]| OsString::from_vec(b.to_vec());
        let params = parse_params(
            [
                os("diff"),
                bytes(b"-L\xffold"),
                bytes(b"--label=\xfenew"),
                os("foo"),
                os("bar"),
            ]
            .iter()
            .cloned()
            .peekable(),
        )
        .unwrap();
        assert_eq!(params.from_label, Some(bytes(b"\xffold")));
        assert_eq!(params.to_label, Some(bytes(b"\xfenew")));
        assert_eq!(params.from, os("foo"));
        assert_eq!(params.to, os("bar"));
    }

    #[rstest]
    #[case::auto(&["--color"], ColorWhen::Auto)]
    #[case::never(&["--color=never"], ColorWhen::Never)]
    #[case::always_after_palette(&["--palette", "ad=35", "--color=always"], ColorWhen::Always)]
    fn color(#[case] args: &[&str], #[case] expected: ColorWhen) {
        let args = [args, &["foo", "bar"]].concat();
        assert_eq!(parse(&args).unwrap().color, expected);
    }

    #[rstest]
    #[case::separate(&["--palette", "ad=35", "--color=always"], "ad=35")]
    #[case::equals(&["--palette=hd=4"], "hd=4")]
    fn palette(#[case] args: &[&str], #[case] expected: &str) {
        let args = [args, &["foo", "bar"]].concat();
        assert_eq!(parse(&args).unwrap().palette.as_deref(), Some(expected));
    }

    #[rstest]
    #[case::show_c_function(&["-p"], Format::Context, &[C_FUNCTION_PATTERN])]
    #[case::show_c_function_unified(&["-u", "-p"], Format::Unified, &[C_FUNCTION_PATTERN])]
    #[case::several_patterns(
        &["-F", "^fn", "-Fx", "--show-function-line=^#", "-e"],
        Format::Ed,
        &["^fn", "x", "^#"],
    )]
    // -F alone selects no output format.
    #[case::no_format(&["-F", "^fn"], Format::Normal, &["^fn"])]
    fn function_lines(#[case] args: &[&str], #[case] format: Format, #[case] patterns: &[&str]) {
        let args = [args, &["foo", "bar"]].concat();
        let params = parse(&args).unwrap();
        assert_eq!(params.format, format);
        let patterns: Vec<Bre> = patterns
            .iter()
            .map(|pattern| Bre::new(pattern).unwrap())
            .collect();
        assert_eq!(params.function_line_patterns, patterns);
    }

    #[rstest]
    #[case::between_operands(&["-u", "old", "-r", "new"], &["-u", "-r"])]
    // An option argument equal to an operand stays with its option.
    #[case::argument_equal_to_operand(&["-F", "new", "old", "new"], &["-F", "new"])]
    #[case::double_dash(&["-q", "--", "-old", "new"], &["-q", "--"])]
    #[case::after_stdin(&["-", "new", "-t"], &["-t"])]
    fn option_arguments(#[case] args: &[&str], #[case] expected: &[&str]) {
        let (_, options) = super::parse_params(
            std::iter::once("diff")
                .chain(args.iter().copied())
                .map(os)
                .peekable(),
        )
        .unwrap();
        assert_eq!(
            options,
            expected.iter().copied().map(os).collect::<Vec<_>>()
        );
    }

    #[rstest]
    #[case::short_separate(&["-S", "mid"])]
    #[case::short_attached(&["-Smid"])]
    #[case::long_equals(&["--starting-file=mid"])]
    #[case::long_separate(&["--starting-file", "mid"])]
    fn starting_file(#[case] args: &[&str]) {
        let args = [args, &["foo", "bar"]].concat();
        let params = parse(&args).unwrap();
        assert_eq!(params.starting_file, Some(os("mid")));
        assert_eq!(params.from, os("foo"));
    }

    // The last of --ignore-file-name-case and --no-ignore-file-name-case
    // wins.
    #[rstest]
    #[case::default(&[], false)]
    #[case::ignore(&["--ignore-file-name-case"], true)]
    #[case::no_ignore(&["--no-ignore-file-name-case"], false)]
    #[case::ignore_then_no_ignore(&["--ignore-file-name-case", "--no-ignore-file-name-case"], false)]
    #[case::no_ignore_then_ignore(&["--no-ignore-file-name-case", "--ignore-file-name-case"], true)]
    fn ignore_file_name_case(#[case] args: &[&str], #[case] expected: bool) {
        let args = [args, &["foo", "bar"]].concat();
        assert_eq!(parse(&args).unwrap().ignore_file_name_case, expected);
    }

    #[test]
    fn ignore_blank_lines() {
        for arg in ["-B", "--ignore-blank-lines"] {
            let params = parse_params(
                [os("diff"), os(arg), os("foo"), os("bar")]
                    .into_iter()
                    .peekable(),
            )
            .unwrap();
            assert!(params.ignore_blank_lines);
        }
    }
}
