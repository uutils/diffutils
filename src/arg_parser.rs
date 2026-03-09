#![allow(unused)]
// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

//! This is a generic parser for program arguments (operands and options).
//!
//! The [Parser] is able to parse the options of all diffutils, e.g. `cmp --options` or `diff --options`.
//!
//! Features:
//!
//! - Allows options to be abbreviated, e.g. \--wi instead of \--width
//! - Allows input like in GNU utils, e.g. the following are all identical:
//!   - `diff --ignore-case --minimal --width=50 file_a file_b`
//!   - `diff --ignore-case --minimal --width 50 file_a file_b`
//!   - `diff -i -d -w 50 file_a file_b`
//!   - `diff -id -w50 file_a file_b`
//!   - `diff -idw50 file_a file_b`
//! - A [NumberParser] is available, which parses option arguments
//!   with optional byte units, e.g. =1024 or =1024KiB
//! - Default handling for \--version and \--help
//! - Returns the [ParsedOption]s or a [ParseError] Enum, which makes it library friendly.
//! - Contains error handling for the typical parsing errors:
//!   - missing and extra operands
//!   - invalid, ambiguous or conflicting options
//!   - missing or not allowed option arguments
//! - Provides error text functions, e.g. add executable and 'Try \--help' to message.
//!
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    fmt::Display,
    iter::Peekable,
};

// TODO finalize copyright
pub const TEXT_COPYRIGHT: &str = r#"Copyright (c) uutils developers
Licenses: MIT License, Apache License 2.0 <https://www.apache.org/licenses/LICENSE-2.0>"#;

// TODO finalize help text footer
pub const TEXT_HELP_FOOTER: &str = r#"
This utility is part of the Rust uutils project: https://github.com/uutils/.
Report bugs here: https://github.com/uutils/diffutils/issues.
"#;

// Version text
#[allow(unused)]
pub const TEXT_VERSION_BASE: &str = concat!("(uutils diffutils) ", env!("CARGO_PKG_VERSION"),);

// AppOption for help, also reacting on -h
pub const OPT_HELP: AppOption = AppOption {
    long_name: "help",
    short: Some('h'),
    has_arg: false,
};
pub const OPT_VERSION: AppOption = AppOption {
    long_name: "version",
    short: Some('v'),
    has_arg: false,
};

/// Add a centralized copyright message to another text.
pub fn add_copyright(text: &str) -> String {
    format!("{text}\n{TEXT_COPYRIGHT}")
}

/// Writes the error message and adds the help hint "Try 'diff \--help' for more information."
///
/// This is the central output function. I affects all utils. \
/// It allows to just use 'eprintln!("{e}");' in case of an error.
pub fn format_error_text<T: Error>(executable: &Executable, error: &T) -> String {
    // for messages the have the executable already
    let exe = format!("{executable}: ");
    let msg = error.to_string();
    if msg.starts_with(&exe) {
        format!("{msg}\n{exe}Try '{executable} --help' for more information.",)
    } else {
        format!("{exe}{msg}\n{exe}Try '{executable} --help' for more information.",)
    }
}

/// Returns the standardized version text for this utility.
pub fn get_version_text(executable: &Executable) -> String {
    format!("{executable} {TEXT_VERSION_BASE}")
}

/// Convert a text into input for the parsers.
///
/// This is for testing and allows to write a simple string `diff file_1 file_2 --width=50`
/// to be converted in the input format the parser expects, like ArgsOs.
#[allow(unused)]
pub fn args_into_peekable_os_strings(args: &str) -> Peekable<std::vec::IntoIter<OsString>> {
    let mut o = Vec::new();
    for arg in args.split(' ') {
        o.push(OsString::from(arg));
    }
    o.into_iter().peekable()
}

/// Check if the user selected an option which is not yet implemented.
#[allow(unused)]
pub fn is_implemented(
    options_parsed: &[ParsedOption],
    implemented_options: &[AppOption],
) -> Result<(), ParseError> {
    if let Some(not_yet) = options_parsed
        .iter()
        .find(|o| implemented_options.contains(o.app_option))
    {
        return Err(ParseError::NotYetImplemented(format!(
            "'--{}' (-{})",
            not_yet.app_option.long_name,
            not_yet.app_option.short.unwrap_or(' ')
        )));
    }

    Ok(())
}

/// This contains the args/options the app allows. They must be all of const value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppOption {
    /// long name of option
    pub long_name: &'static str,
    pub short: Option<char>,
    pub has_arg: bool,
    // pub arg_default: Option<&'static str>,
}

impl AppOption {
    /// formatted long option
    ///
    /// Returns the long name formatted: `--option`. \
    /// There is inconsistency in GNU diffutils, if these are printed with or without quotes.
    pub fn format_long(&self) -> String {
        format!("--{}", self.long_name)
    }

    /// formatted long and short option
    ///
    /// There is inconsistency in GNU diffutils, if these are printed with or without quotes.
    ///
    ///  # Returns
    /// * Some(short): `'--option' (-c)`.
    /// * None: [Self::format_long]
    pub fn format_for_error_msg(&self) -> String {
        self.format_long()
        // match self.short {
        //     Some(c) => format!("--{} (-{c})", self.long_name),
        //     None => self.format_long(),
        // }
    }

    /// formatted option char
    ///
    /// Returns the short char formatted: "-c" or an empty String if None.
    #[allow(unused)]
    pub fn short_or_empty(&self) -> String {
        match self.short {
            Some(c) => format!("-{c}"),
            None => String::new(),
        }
    }
}

/// One parsed option.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedOption {
    pub app_option: &'static AppOption,
    /// Argument of the option as string_lossy, e.g. the "1000kB" of "\--bytes=1000kB".
    pub arg_for_option: Option<String>,
    /// Argument of the option as original OsString
    pub arg_for_option_os: Option<OsString>,
    /// If the user typed the long name or used the short char to set the option.
    pub name_type_used: OptionNameTypeUsed,
}

impl ParsedOption {
    pub fn new(
        app_option: &'static AppOption,
        arg_for_option_os: OsString,
        name_type_used: OptionNameTypeUsed,
    ) -> Self {
        Self {
            app_option,
            arg_for_option: Some(arg_for_option_os.to_string_lossy().to_string()),
            arg_for_option_os: Some(arg_for_option_os),
            name_type_used,
        }
    }

    /// Create an option which does not have an argument.
    pub fn new_no_arg(app_option: &'static AppOption, used: OptionNameTypeUsed) -> Self {
        Self {
            app_option,
            arg_for_option: None,
            arg_for_option_os: None,
            name_type_used: used,
        }
    }

    /// This checks if an option requires an argument and if it already known.
    ///
    /// * Case A: `--long-option=argument`: Argument is already parsed
    /// * Case B: `--long-option argument`: Argument must be the next in the given args
    /// * Case C: `-bArgument`: Argument is already parsed
    /// * Case D: `-b Argument`: Argument must be the next in the given args
    fn check_add_arg<I: Iterator<Item = OsString>>(
        &mut self,
        opts: &mut Peekable<I>,
    ) -> Result<(), ParseError> {
        // argument missing
        if self.app_option.has_arg {
            if self.arg_for_option.is_none() {
                // take following argument if it is not an option
                if let Some(arg) = opts.peek() {
                    let arg = arg.to_string_lossy();
                    if !arg.starts_with('-') {
                        self.arg_for_option = Some(arg.to_string());
                        _ = opts.next();
                    }
                }
                if self.arg_for_option.is_none() {
                    return Err(ParseError::ArgForOptionMissing(self.clone()));
                }
            }
        } else {
            // argument allowed?
            if self.arg_for_option.is_some() {
                return Err(ParseError::ArgForOptionNotAllowed(self.clone()));
            }
        }

        Ok(())
    }

    /// Sets arg_for_option_os and arg_for_option as string_lossy.
    pub fn set_arg_for_option(&mut self, arg_for_option_os: OsString) {
        self.arg_for_option = Some(arg_for_option_os.to_string_lossy().to_string());
        self.arg_for_option_os = Some(arg_for_option_os);
    }

    /// Easy String conversion: returns the Argument or an empty String if None.
    pub fn arg_for_option_or_empty_string(&self) -> String {
        match &self.arg_for_option {
            Some(s) => s.clone(),
            None => String::new(),
        }
    }
}

impl Default for ParsedOption {
    fn default() -> Self {
        Self {
            app_option: &AppOption {
                long_name: "dummy",
                short: None,
                has_arg: false,
            },
            arg_for_option: None,
            arg_for_option_os: None,
            name_type_used: OptionNameTypeUsed::LongName,
        }
    }
}

/// To differentiate the user input, did he use -s or \--silent.
/// While this is technically no difference, the error message may vary.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum OptionNameTypeUsed {
    #[default]
    None,
    LongName,
    ShortName,
}

/// This is a generic parser for program arguments (operands and options),
/// but without the executable.
///
/// This generic parser is able to parse the options of all diffutils, e.g. `cmp --options` or `diff --options`. \
/// The allowed options are passed as a list of static [AppOption]s, as they are known at compile time.
///
/// # Example: read params for sdiff
/// ```rust
/// # use diffutilslib::sdiff::{sdiff, SDiffOk, TEXT_HELP};
/// # use diffutilslib::sdiff::params_sdiff;
/// let args = "sdiff --help";
/// // let args = "sdiff file_1.txt file_2.txt --width=40";
/// // Test helper conversion, usually this is ArgsOs.
/// let args = diffutilslib::arg_parser::args_into_peekable_os_strings(&args);
/// let params = match sdiff(args) {
///     Ok(res) => match res {
///         SDiffOk::Different => todo!(),
///         SDiffOk::Equal => todo!(),
///         SDiffOk::Help => {
///             println!("{TEXT_HELP}");
///             return; // ExitCode::from(0);
///         }
///         SDiffOk::Version => todo!(),
///     },
///     Err(e) => {
///         eprintln!("{e}");
///         return; // ExitCode::from(2);
///     }
/// };
/// ```
#[derive(Debug, Default)]
pub struct Parser {
    pub options_parsed: Vec<ParsedOption>,
    pub operands: Vec<OsString>,
    // temporary stored for each param
    name_type_used: OptionNameTypeUsed,
}

impl Parser {
    /// Parse the args into operands and options for the utility.
    ///
    /// The arguments must not contain the executable.
    ///
    /// The allowed options are passed as a list of static [AppOption]s, as they are known at compile time.
    ///
    /// # Returns Result
    /// * Ok: [Parser] with [ParsedOption]s and operands (file names)
    /// * Error: [ParseError]
    pub fn parse_params<I: Iterator<Item = OsString>>(
        app_options: &'static [AppOption],
        mut args: Peekable<I>,
    ) -> Result<Parser, ParseError> {
        // sdiff options begin with ‘-’, so normally from-file and to-file may not begin with ‘-’.
        // However, -- as an argument by itself treats the remaining arguments as file names even if they begin with ‘-’.
        // You may not use - as an input file.
        // read next param as file name, here we generally use read as operand
        let mut parser = Self::default();
        let mut is_double_dash = false;
        while let Some(param_os) = args.next() {
            let mut param = param_os.to_string_lossy().to_string();
            // dbg!(&param);
            let mut ci = param.char_indices().peekable();
            let (_, c0) = ci.next().expect("Param must have at least one char!");
            // is param?
            if c0 != '-' || param == "-" || is_double_dash {
                // Operand, not an option with - or --
                // or single dash '-', this is for file as StandardInput
                parser.operands.push(param_os);
                continue;
            }
            // check 2nd char, which must exist, see above checks
            let (_, c1) = ci.next().unwrap();
            let mut p_opt = ParsedOption::default();
            // has 3rd char?
            if let Some((pos_c2, _c2)) = ci.peek() {
                if c1 == '-' {
                    // long option, e.g. --bytes
                    parser.name_type_used = OptionNameTypeUsed::LongName;

                    // Find argument for some options, either '=' or following arg.
                    // This also shortens param to its name.
                    if let Some(p) = param[*pos_c2..].find('=') {
                        // only --bytes and --ignore-initial must have bytes, else return error
                        // reduce param to option and
                        // return bytes without = sign.
                        let os = Self::split_os_prefix(&param_os, p + *pos_c2 + 1)?;
                        p_opt.set_arg_for_option(os);
                        param = param[0..p + *pos_c2].to_string();
                    }

                    // allow partial option descriptors, like --he for --help, if unique
                    p_opt.app_option =
                        Self::identify_option_from_partial_text(&param_os, app_options)?;

                    p_opt.name_type_used = OptionNameTypeUsed::LongName;
                    p_opt.check_add_arg(&mut args)?;
                    parser.options_parsed.push(p_opt);
                } else {
                    // -MultiSingleChar, e.g. -bl or option with bytes -n200
                    parser.name_type_used = OptionNameTypeUsed::ShortName;
                    let mut c = c1;
                    let mut pos = 1;
                    loop {
                        let Some(opt) = app_options.iter().find(|o| o.short == Some(c)) else {
                            return Err(ParseError::InvalidOption(param_os));
                        };
                        if opt.has_arg {
                            // take rest of the string as arg
                            let arg_for_option_os = if param.len() > pos + 1 {
                                Some(Self::split_os_prefix(&param_os, pos + 1)?)
                            } else {
                                args.next()
                            };
                            let Some(os) = arg_for_option_os else {
                                return Err(ParseError::ArgForOptionMissing(
                                    ParsedOption::new_no_arg(opt, OptionNameTypeUsed::ShortName),
                                ));
                            };
                            parser.options_parsed.push(ParsedOption::new(
                                opt,
                                os,
                                OptionNameTypeUsed::ShortName,
                            ));
                            break;
                        } else {
                            parser
                                .options_parsed
                                .push(ParsedOption::new_no_arg(opt, OptionNameTypeUsed::ShortName));
                        }
                        match ci.next() {
                            Some((p, cx)) => {
                                c = cx;
                                pos = p
                            }
                            None => break,
                        }
                    }
                }
            } else {
                // single short options, e.g. -b.
                parser.name_type_used = OptionNameTypeUsed::ShortName;
                match app_options.iter().find(|opt| {
                    if let Some(c) = opt.short {
                        c == c1
                    } else {
                        false
                    }
                }) {
                    Some(opt) => {
                        p_opt.app_option = opt;
                        p_opt.name_type_used = OptionNameTypeUsed::ShortName;
                        p_opt.check_add_arg(&mut args)?;
                        parser.options_parsed.push(p_opt);
                    }
                    None => {
                        if c1 == '-' {
                            is_double_dash = true
                        } else {
                            return Err(ParseError::InvalidOption(param_os));
                        }
                    }
                }
            }
        }

        // identified unique option
        if parser.is_help() {
            parser.set_only_option(&OPT_HELP);
            return Ok(parser);
        }
        if parser.is_version() {
            parser.set_only_option(&OPT_VERSION);
            return Ok(parser);
        }

        Ok(parser)
    }

    /// * param_os: expected to start with "\--"
    pub fn identify_option_from_partial_text(
        param_os: &OsStr,
        app_options: &'static [AppOption],
    ) -> Result<&'static AppOption, ParseError> {
        assert!(param_os.len() > 2);
        let mut param = &param_os.to_string_lossy()[2..];
        if let Some(p) = param.find('=') {
            param = &param[0..p];
        }
        let l = param.len();
        let possible_opts: Vec<&'static AppOption> = app_options
            .iter()
            .filter(|&it| it.long_name.len() >= l && &it.long_name[0..l] == param)
            .collect();

        match possible_opts.len() {
            0 => Err(ParseError::UnrecognizedOption(param_os.to_os_string())),

            1 => Ok(*possible_opts.first().unwrap()),

            _ => Err(ParseError::AmbiguousOption(
                param_os.to_os_string(),
                possible_opts,
            )),
        }
    }

    /// Check if user requested the \--help output.
    pub fn is_help(&self) -> bool {
        self.options_parsed
            .iter()
            .any(|opt| *opt.app_option == OPT_HELP)
    }

    /// Check if user requested the \--version output.
    pub fn is_version(&self) -> bool {
        self.options_parsed
            .iter()
            .any(|opt| *opt.app_option == OPT_VERSION)
    }

    fn set_only_option(&mut self, option: &'static AppOption) {
        self.options_parsed = vec![ParsedOption::new_no_arg(option, self.name_type_used)];
        self.operands.clear();
    }

    /// Split an OsString on Linux. On Windows this is not possible. \
    /// This is required for options like `--file-name=argument-non-utf-8`
    ///
    /// # Returns
    /// * A slice of the OsStr starting from `index`.
    /// * None if the OS doesn't support byte-slicing or index is out of bounds.
    pub fn split_os_prefix(os_str: &OsStr, index: usize) -> Result<OsString, ParseError> {
        #[cfg(unix)]
        {
            // On Unix, OsStr is just a sequence of bytes (often UTF-8, but not guaranteed).
            use std::os::unix::ffi::OsStrExt;
            let bytes = os_str.as_bytes();
            if index <= bytes.len() {
                return Ok(OsStr::from_bytes(&bytes[index..]).to_os_string());
            }
        }

        #[cfg(not(unix))]
        {
            // On Windows/others, we can't safely slice raw bytes because
            // they use Wtf-8/Utf-16 which might split a surrogate pair.
            // We fall back to UTF-8 conversion if possible.
            let r = os_str.to_str().and_then(|s| {
                if index <= s.len() {
                    Some(OsString::from(&s[index..]))
                } else {
                    None
                }
            });
            if let Some(os) = r {
                return Ok(os);
            }
        }

        Err(ParseError::NoUnicode(os_str.to_os_string()))
    }
}

/// Contains all parser errors and their text messages.
///
/// All errors can be output easily using the normal Display functionality.
/// To format the error message for the typical diffutils output, use [format_error_text].
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// When the long option is abbreviated, but does not have a unique match.
    /// (ambiguous option, possible options)
    AmbiguousOption(OsString, Vec<&'static AppOption>),

    /// 'executable': option '--silent' doesn't allow an argument
    /// (wrong option)
    ArgForOptionNotAllowed(ParsedOption),

    /// (option, short or long name used)
    ArgForOptionMissing(ParsedOption),

    /// Having more operands than allowed (usually 2)
    /// (wrong operand)
    ExtraOperand(OsString),

    /// Non-existent single dash option.
    /// (unidentified option)
    InvalidOption(OsString),

    /// number for an option argument incorrect
    InvalidValueNumber(ParsedOption),
    InvalidValueNumberUnit(ParsedOption),
    InvalidValueNumberOverflow(ParsedOption),

    /// 'executable' as first parameter missing.
    #[allow(unused)] // Allow usage for main function so all parsing errors are covered.
    NoExecutable,

    /// no args for the actual utility given
    NoOperands(Executable),

    /// Parsed option is not in unicode.
    /// Since Rust cannot split OsString on Non-Linux Systems,
    /// it can accept the argument for an option only as
    /// separate arg (--regex someRegex).
    NoUnicode(OsString),

    /// Two options cannot be used together, e.g. cmp --silent and --verbose (output).
    OptionsIncompatible(&'static AppOption, &'static AppOption),

    /// Non-existent long option. This is "unrecognized" because the name can be abbreviated.
    /// (unrecognized option)
    UnrecognizedOption(OsString),

    NotYetImplemented(String),
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::AmbiguousOption(param, possible_opts) => {
                // create list of possible options
                let mut list = Vec::new();
                for opt in possible_opts {
                    list.push(opt.format_long());
                }
                write!(
                    f,
                    "option '{}' is ambiguous; possibilities: {}",
                    param.to_string_lossy(),
                    list.join(" ")
                )
            }
            ParseError::ArgForOptionNotAllowed(opt) => write!(
                f,
                "option {} doesn't allow an argument",
                opt.app_option.format_long()
            ),
            ParseError::ArgForOptionMissing(opt) => {
                write!(
                    f,
                    "option {} requires an argument",
                    opt.app_option.format_for_error_msg(),
                )
            }
            ParseError::ExtraOperand(opt) => write!(f, "extra operand '{}'", opt.to_string_lossy()),
            ParseError::InvalidValueNumber(opt) | ParseError::InvalidValueNumberUnit(opt) => {
                write!(
                    f,
                    "invalid {} value '{}'",
                    // "invalid argument '{}' for '--{}'{}",
                    opt.app_option.format_for_error_msg(),
                    opt.arg_for_option_or_empty_string(),
                    // opt.short_char_or_empty_string(),
                )
            }
            ParseError::InvalidValueNumberOverflow(opt) => {
                write!(
                    f,
                    "invalid {} value '{}' (too large)",
                    // "invalid argument '{}' for '--{}'{}",
                    opt.app_option.format_for_error_msg(),
                    opt.arg_for_option_or_empty_string(),
                    // opt.short_char_or_empty_string(),
                )
            }
            ParseError::InvalidOption(param) => {
                write!(
                    f,
                    "{}",
                    &format!("invalid option '{}'", param.to_string_lossy())
                )
            }
            ParseError::NoExecutable => {
                write!(f, "Expected utility name as second argument, got nothing.")
            }
            ParseError::NoOperands(exe) => {
                write!(f, "missing operand after '{exe}'")
            }
            ParseError::NoUnicode(os) => {
                let mut s = OsString::from("Cannot parse non-unicode '");
                s.push(os);
                s.push(OsString::from(
                    "'. Separate the argument from the option, e.g. '--from-file argument' instead '--from-file=argument'",
                ));
                write!(f, "Expected utility name as second argument, got nothing.")
            }
            ParseError::OptionsIncompatible(op_1, op_2) => {
                write!(
                    f,
                    "options {} and {} are incompatible",
                    op_1.format_for_error_msg(),
                    op_2.format_for_error_msg()
                )
            }
            ParseError::UnrecognizedOption(param) => {
                write!(
                    f,
                    "{}",
                    &format!("unrecognized option '{}'", param.to_string_lossy())
                )
            }
            ParseError::NotYetImplemented(param) => {
                write!(f, "{}", &format!("not yet implemented: option {param}"))
            }
        }
    }
}

pub struct NumberParser {}

impl NumberParser {
    /// Parses a number with an optional unit, e.g. 10MiB.
    ///
    /// Follows <https://www.gnu.org/software/diffutils/manual/html_node/cmp-Options.html>.
    pub fn parse_number(parsed_option: &ParsedOption) -> Result<u64, ParseError> {
        let Some(num_unit) = &parsed_option.arg_for_option else {
            return Err(ParseError::InvalidValueNumber(parsed_option.clone()));
        };
        if num_unit.is_empty() {
            return Err(ParseError::InvalidValueNumber(parsed_option.clone()));
        }

        // split number and unit, parse unit
        let multiplier: u64;
        let n = match num_unit.find(|b: char| !b.is_ascii_digit()) {
            Some(pos) => {
                if pos == 0 {
                    return Err(ParseError::InvalidValueNumber(parsed_option.clone()));
                }
                multiplier = match Self::parse_number_unit(&num_unit[pos..]) {
                    Some(m) => m,
                    None => return Err(ParseError::InvalidValueNumberUnit(parsed_option.clone())),
                };
                &num_unit[0..pos]
            }
            None => {
                multiplier = 1;
                num_unit
            }
        };

        // return value
        match n.parse::<u64>() {
            Ok(num) => {
                if multiplier == 1 {
                    Ok(num)
                } else {
                    match num.checked_mul(multiplier) {
                        Some(r) => Ok(r),
                        None => Err(ParseError::InvalidValueNumberOverflow(
                            parsed_option.clone(),
                        )),
                    }
                }
            }
            // This is an additional error message not present in GNU DiffUtils.
            Err(e) if *e.kind() == std::num::IntErrorKind::PosOverflow => Err(
                ParseError::InvalidValueNumberOverflow(parsed_option.clone()),
            ),
            Err(_) => Err(ParseError::InvalidValueNumber(parsed_option.clone())),
        }
    }

    /// Parses a number unit, e.g. "KiB" into a multiplier
    /// which then can be used to calculate the final number of bytes.
    ///
    /// # Returns
    /// A multiplier depending on the given unit, e.g. 'KiB' -> 1024
    /// or None if unit could not be identified.
    ///
    /// Units up eo Exabyte (EiB) following GNU documentation: \
    /// <https://www.gnu.org/software/diffutils/manual/html_node/cmp-Options.html>.
    #[cfg(not(feature = "feat_allow_case_insensitive_number_units"))]
    // #[allow(unused)] // required for cmp
    pub fn parse_number_unit(unit: &str) -> Option<u64> {
        let multiplier = match unit {
            "kB" | "KB" => 1_000,
            "k" | "K" | "KiB" | "kiB" => 1_024,
            "MB" => 1_000_000,
            "M" | "MiB" => 1_048_576,
            "GB" => 1_000_000_000,
            "G" | "GiB" => 1_073_741_824,

            "TB" => 1_000_000_000_000,
            "T" | "TiB" => 1_099_511_627_776,
            "PB" => 1_000_000_000_000_000,
            "P" | "PiB" => 1_125_899_906_842_624,
            "EB" => 1_000_000_000_000_000_000,
            "E" | "EiB" => 1_152_921_504_606_846_976,

            // Everything above EiB cannot fit into u64.
            // GNU cmp just returns an invalid bytes value
            // "ZB" => 1_000_000_000_000_000_000_000,
            // "Z" | "ZiB" => 1_180_591_620_717_411_303_424,
            // "YB" => 1_000_000_000_000_000_000_000_000,
            // "Y" | "YiB" => 1_208_925_819_614_629_174_706_176,
            _ => {
                return None;
            }
        };

        Some(multiplier)
    }

    /// Returns a multiplier depending on the given unit, e.g. 'KiB' -> 1024,
    /// which then can be used to calculate the final number of bytes.
    /// Following GNU documentation: https://www.gnu.org/software/diffutils/manual/html_node/cmp-Options.html
    /// TODO case
    #[cfg(feature = "feat_allow_case_insensitive_number_units")]
    pub fn parse_number_unit(unit: &str) -> Option<u64> {
        // Note that GNU cmp advertises supporting up to Y, but fails if you try
        // to actually use anything beyond E.
        let unit = unit.to_owned().to_ascii_lowercase();
        // .to_ascii_lowercase().as_str();
        let multiplier = match unit.as_str() {
            "kb" => 1_000,
            "k" | "kib" => 1_024,
            "mb" => 1_000_000,
            "m" | "mib" => 1_048_576,
            "gb" => 1_000_000_000,
            "g" | "gib" => 1_073_741_824,

            "tb" => 1_000_000_000_000,
            "t" | "tib" => 1_099_511_627_776,
            "pb" => 1_000_000_000_000_000,
            "p" | "pib" => 1_125_899_906_842_624,
            "eb" => 1_000_000_000_000_000_000,
            "e" | "eib" => 1_152_921_504_606_846_976,

            // Everything above EiB cannot fit into u64.
            // GNU cmp just returns an invalid bytes value
            // "zb" => 1_000_000_000_000_000_000_000,
            // "z" | "zib" => 1_180_591_620_717_411_303_424,
            // "yb" => 1_000_000_000_000_000_000_000_000,
            // "y" | "yib" => 1_208_925_819_614_629_174_706_176,
            _ => {
                return None;
            }
        };

        Some(multiplier)
    }
}

/// Differentiates the utilities included in DiffUtil
/// and replaces executable as OsString.
///
/// This allows easy output of the executable name with
/// ```format!("{}", params.executable)```
/// without calling ```to_string_lossy()``` each time.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Executable {
    Cmp,
    Diff,
    Diff3,
    Patch,
    SDiff,

    // Called from a library. Stores name.
    NotRecognized(OsString),
}

#[allow(unused)]
impl Executable {
    /// Returns the executable name as OsString. \
    ///
    /// In case of [Self::NotRecognized], this is the original OsString.
    ///
    /// The name is mostly used to write it which always requires a String.
    pub fn executable(&self) -> OsString {
        match self {
            Executable::NotRecognized(os_string) => os_string.clone(),
            _ => OsString::from(self.to_string()),
        }
    }

    /// Return as OsString. Same as fn [Self::executable].
    pub fn to_os_string(&self) -> OsString {
        self.executable()
    }

    /// Read the first arg (the executable) without moving the iterator of args.
    ///
    /// Returns
    /// - Some: [Executable].
    ///   - Diffutils: diff, cmp, sdiff, diff3 and patch
    ///   - NotRecognized(OsString) for all other inputs
    /// - None: only if no argument was found.
    pub fn from_args_os<I: Iterator<Item = OsString>>(
        args: &mut Peekable<I>,
        move_iter: bool,
    ) -> Option<Self> {
        if move_iter {
            args.next().map(|exe| Self::from(&exe))
        } else {
            args.peek().map(Self::from)
        }
    }
}

impl From<&OsString> for Executable {
    fn from(executable: &OsString) -> Self {
        match executable.to_str() {
            Some("cmp") => Executable::Cmp,
            Some("diff") => Executable::Diff,
            Some("diff3") => Executable::Diff3,
            Some("patch") => Executable::Patch,
            Some("sdiff") => Executable::SDiff,
            _ => Executable::NotRecognized(OsString::from(executable)),
        }
    }
}

impl Display for Executable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Executable::Cmp => "cmp",
            Executable::Diff => "diff",
            Executable::Diff3 => "diff3",
            Executable::SDiff => "sdiff",
            Executable::Patch => "patch",
            Executable::NotRecognized(name) => &name.to_string_lossy(),
        };
        write!(f, "{name}")
    }
}
