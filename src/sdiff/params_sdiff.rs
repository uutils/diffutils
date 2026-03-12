// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

//! This module contains the Parser for sdiff arguments ([ParamsSDiff::parse_params]).
use std::{ffi::OsString, iter::Peekable};

use crate::arg_parser::{
    AppOption, Executable, ParseError, ParsedOption, Parser, OPT_HELP, OPT_VERSION,
};

// use crate::{
//     arg_parser::{self, AppOption, Executable, ParseError, ParsedOption, OPT_HELP, OPT_VERSION},
//     sdiff::params_sdiff::ParamsSDiff,
// };

pub type ResultSDiffParse = Result<SDiffParseOk, ParseError>;

// AppOptions for sdiff
pub const OPT_DIFF_PROGRAM: AppOption = AppOption {
    long_name: "diff-program",
    short: None,
    has_arg: true,
};
pub const OPT_EXPAND_TABS: AppOption = AppOption {
    long_name: "expand-tabs",
    short: Some('t'),
    has_arg: false,
};
pub const OPT_IGNORE_ALL_SPACE: AppOption = AppOption {
    long_name: "ignore-all-space",
    short: Some('W'),
    has_arg: false,
};
pub const OPT_IGNORE_BLANK_LINES: AppOption = AppOption {
    long_name: "ignore-blank-lines",
    short: Some('B'),
    has_arg: false,
};
pub const OPT_IGNORE_CASE: AppOption = AppOption {
    long_name: "ignore-case",
    short: Some('i'),
    has_arg: false,
};
pub const OPT_IGNORE_MATCHING_LINES: AppOption = AppOption {
    long_name: "ignore-matching-lines",
    short: Some('I'),
    has_arg: true,
};
pub const OPT_IGNORE_SPACE_CHANGE: AppOption = AppOption {
    long_name: "ignore-space-change",
    short: Some('b'),
    has_arg: false,
};
pub const OPT_IGNORE_TAB_EXPANSION: AppOption = AppOption {
    long_name: "ignore-tab-expansion",
    short: Some('E'),
    has_arg: false,
};
pub const OPT_IGNORE_TRAILING_SPACE: AppOption = AppOption {
    long_name: "ignore-trailing-space",
    short: Some('Z'),
    has_arg: false,
};
pub const OPT_LEFT_COLUMN: AppOption = AppOption {
    long_name: "left-column",
    short: Some('l'),
    has_arg: false,
};
pub const OPT_MINIMAL: AppOption = AppOption {
    long_name: "minimal",
    short: Some('d'),
    has_arg: false,
};
pub const OPT_OUTPUT: AppOption = AppOption {
    long_name: "output",
    short: Some('o'),
    has_arg: true,
};
pub const OPT_SPEED_LARGE_FILES: AppOption = AppOption {
    long_name: "speed-large-files",
    short: Some('H'),
    has_arg: false,
};
pub const OPT_STRIP_TRAILING_CR: AppOption = AppOption {
    long_name: "strip-trailing-cr",
    short: None,
    has_arg: false,
};
pub const OPT_SUPPRESS_COMMON_LINES: AppOption = AppOption {
    long_name: "suppress-common-lines",
    short: Some('s'),
    has_arg: false,
};
pub const OPT_TABSIZE: AppOption = AppOption {
    long_name: "tabsize",
    short: None,
    has_arg: true,
};
pub const OPT_TEXT: AppOption = AppOption {
    long_name: "text",
    short: Some('a'),
    has_arg: false,
};
pub const OPT_WIDTH: AppOption = AppOption {
    long_name: "width",
    short: Some('w'),
    has_arg: true,
};

// Array for ArgParser
pub const APP_OPTIONS: [AppOption; 20] = [
    OPT_DIFF_PROGRAM,
    OPT_EXPAND_TABS,
    OPT_HELP,
    OPT_IGNORE_ALL_SPACE,
    OPT_IGNORE_BLANK_LINES,
    OPT_IGNORE_CASE,
    OPT_IGNORE_MATCHING_LINES,
    OPT_IGNORE_SPACE_CHANGE,
    OPT_IGNORE_TAB_EXPANSION,
    OPT_IGNORE_TRAILING_SPACE,
    OPT_LEFT_COLUMN,
    OPT_MINIMAL,
    OPT_OUTPUT,
    OPT_SPEED_LARGE_FILES,
    OPT_STRIP_TRAILING_CR,
    OPT_SUPPRESS_COMMON_LINES,
    OPT_TABSIZE,
    OPT_TEXT,
    OPT_VERSION,
    OPT_WIDTH,
];

// These options throw an error, rather than go unnoticed.
#[cfg(feature = "feat_check_not_yet_implemented")]
pub const NOT_YET_IMPLEMENTED: [AppOption; 15] = [
    OPT_DIFF_PROGRAM,
    OPT_IGNORE_ALL_SPACE,
    OPT_IGNORE_BLANK_LINES,
    OPT_IGNORE_CASE,
    OPT_IGNORE_MATCHING_LINES,
    OPT_IGNORE_SPACE_CHANGE,
    OPT_IGNORE_TAB_EXPANSION,
    OPT_IGNORE_TRAILING_SPACE,
    OPT_LEFT_COLUMN,
    OPT_MINIMAL,
    OPT_OUTPUT,
    OPT_SPEED_LARGE_FILES,
    OPT_STRIP_TRAILING_CR,
    OPT_SUPPRESS_COMMON_LINES,
    OPT_TEXT,
];

/// Parser Result Ok Enum with Params.
///
/// # Returns
/// - Params in normal cases
/// - Just Help or Version when these are requested as the params are then not relevant.
///
/// Error will be returned as [ParseError] in the function Result Error.
#[derive(Debug, PartialEq)]
pub enum SDiffParseOk {
    Params(ParamsSDiff),
    Help,
    Version,
}

/// Holds the given command line arguments except "--version" and "--help".
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParamsSDiff {
    /// Identifier
    pub executable: Executable,
    pub from: OsString,
    pub to: OsString,
    /// --diff-program=PROGRAM   use PROGRAM to compare files
    pub diff_program: Option<String>,
    /// -t, --expand-tabs            expand tabs to spaces in output
    pub expand_tabs: bool,
    /// --help                   display this help and exit
    pub help: bool,
    /// -W, --ignore-all-space       ignore all white space
    pub ignore_all_space: bool,
    /// -B, --ignore-blank-lines     ignore changes whose lines are all blank
    pub ignore_blank_lines: bool,
    /// -i, --ignore-case            consider upper- and lower-case to be the same
    pub ignore_case: bool,
    /// -I, --ignore-matching-lines=REGEXP  ignore changes all whose lines match REGEXP
    pub ignore_matching_lines: Option<String>,
    /// -b, --ignore-space-change    ignore changes in the amount of white space
    pub ignore_space_change: bool,
    /// -E, --ignore-tab-expansion   ignore changes due to tab expansion
    pub ignore_tab_expansion: bool,
    /// -Z, --ignore-trailing-space  ignore white space at line end
    pub ignore_trailing_space: bool,
    /// -l, --left-column            output only the left column of common lines
    pub left_column: bool,
    /// -d, --minimal                try hard to find a smaller set of changes
    pub minimal: bool,
    /// -o, --output=FILE            operate interactively, sending output to FILE
    pub output: Option<String>,
    /// -H, --speed-large-files      assume large files, many scattered small changes
    pub speed_large_files: bool,
    /// --strip-trailing-cr      strip trailing carriage return on input
    pub strip_trailing_cr: bool,
    /// -s, --suppress-common-lines  do not output common lines
    pub suppress_common_lines: bool,
    /// --tabsize=NUM            tab stops at every NUM (default 8) print columns
    pub tabsize: usize,
    /// -a, --text                   treat all files as text
    pub text: bool,
    /// -v, --version                output version information and exit
    pub version: bool,
    /// -w, --width=NUM              output at most NUM (default 130) print columns
    pub width: usize,
}

impl Default for ParamsSDiff {
    fn default() -> Self {
        Self {
            executable: Executable::SDiff,
            from: Default::default(),
            to: Default::default(),
            diff_program: Default::default(),
            expand_tabs: Default::default(),
            help: Default::default(),
            ignore_all_space: Default::default(),
            ignore_blank_lines: Default::default(),
            ignore_case: Default::default(),
            ignore_matching_lines: Default::default(),
            ignore_space_change: Default::default(),
            ignore_tab_expansion: Default::default(),
            ignore_trailing_space: Default::default(),
            left_column: Default::default(),
            minimal: Default::default(),
            output: Default::default(),
            speed_large_files: Default::default(),
            strip_trailing_cr: Default::default(),
            suppress_common_lines: Default::default(),
            tabsize: 8,
            text: Default::default(),
            version: Default::default(),
            width: 130,
        }
    }
}

impl ParamsSDiff {
    /// Parses the program arguments.
    ///
    /// The arguments must not contain the executable.
    pub fn parse_params<I: Iterator<Item = OsString>>(
        executable: &Executable,
        args: Peekable<I>,
    ) -> ResultSDiffParse {
        let parser = Parser::parse_params(&APP_OPTIONS, args)?;

        // check implemented options
        #[cfg(feature = "feat_check_not_yet_implemented")]
        {
            crate::arg_parser::is_implemented(&parser.options_parsed, &NOT_YET_IMPLEMENTED)?;
        }

        let mut params = Self {
            executable: executable.clone(),
            ..Default::default()
        };

        // set options
        for parsed_option in &parser.options_parsed {
            // dbg!(parsed_option);
            match *parsed_option.app_option {
                OPT_DIFF_PROGRAM => params.diff_program = parsed_option.arg_for_option.clone(),
                OPT_EXPAND_TABS => params.expand_tabs = true,
                OPT_HELP => return Ok(SDiffParseOk::Help),
                OPT_IGNORE_ALL_SPACE => params.ignore_all_space = true,
                OPT_IGNORE_BLANK_LINES => params.ignore_blank_lines = true,
                OPT_IGNORE_CASE => params.ignore_case = true,
                OPT_IGNORE_MATCHING_LINES => {
                    params.ignore_matching_lines = parsed_option.arg_for_option.clone()
                }
                OPT_IGNORE_SPACE_CHANGE => params.ignore_space_change = true,
                OPT_IGNORE_TAB_EXPANSION => params.ignore_tab_expansion = true,
                OPT_IGNORE_TRAILING_SPACE => params.ignore_trailing_space = true,
                OPT_LEFT_COLUMN => params.left_column = true,
                OPT_MINIMAL => params.minimal = true,
                OPT_OUTPUT => params.output = parsed_option.arg_for_option.clone(),
                OPT_SPEED_LARGE_FILES => params.speed_large_files = true,
                OPT_STRIP_TRAILING_CR => params.strip_trailing_cr = true,
                OPT_SUPPRESS_COMMON_LINES => params.suppress_common_lines = true,
                OPT_TABSIZE => {
                    params.set_tabsize(parsed_option)?;
                }
                OPT_TEXT => params.text = true,
                OPT_VERSION => return Ok(SDiffParseOk::Version),
                OPT_WIDTH => {
                    params.set_width(parsed_option)?;
                }

                // This is not an error, but a todo. Unfortunately an Enum is not possible.
                _ => todo!("Err Option: {}", parsed_option.app_option.long_name),
            }
        }

        // set operands
        match parser.operands.len() {
            0 => return Err(ParseError::NoOperands(executable.clone())),
            // If only file_1 is set, then file_2 defaults to '-', so it reads from StandardInput.
            1 => {
                params.from = parser.operands[0].clone();
                params.to = OsString::from("-");
            }
            2 => {
                params.from = parser.operands[0].clone();
                params.to = parser.operands[1].clone();
            }
            _ => {
                return Err(ParseError::ExtraOperand(parser.operands[2].clone()));
            }
        }

        // dbg!(&params);
        Ok(SDiffParseOk::Params(params))
    }

    pub fn set_tabsize(&mut self, parsed_option: &ParsedOption) -> Result<usize, ParseError> {
        let tab_size = parsed_option.arg_for_option.clone().unwrap_or_default();
        let t = match tab_size.parse::<usize>() {
            Ok(w) => w,
            Err(_) => return Err(ParseError::InvalidValueNumber(parsed_option.clone())),
        };
        self.tabsize = t;

        Ok(t)
    }

    pub fn set_width(&mut self, parsed_option: &ParsedOption) -> Result<usize, ParseError> {
        let width = parsed_option.arg_for_option.clone().unwrap_or_default();
        let w = match width.parse::<usize>() {
            Ok(w) => w,
            Err(_) => return Err(ParseError::InvalidValueNumber(parsed_option.clone())),
        };
        self.width = w;

        Ok(w)
    }
}

// Usually assert is used like assert_eq(test result, expected result).
#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> OsString {
        OsString::from(s)
    }

    /// Simplify call of parser, just pass a normal string like in the Terminal.
    fn parse(args: &str) -> ResultSDiffParse {
        let mut o = Vec::new();
        for arg in args.split(' ') {
            o.push(os(arg));
        }
        let mut p = o.into_iter().peekable();
        // remove executable
        let executable = Executable::from_args_os(&mut p, true).unwrap();

        ParamsSDiff::parse_params(&executable, p)
    }

    fn res_ok(params: ParamsSDiff) -> ResultSDiffParse {
        Ok(SDiffParseOk::Params(params))
    }

    #[test]
    fn positional() {
        // file_1 and file_2 given
        assert_eq!(
            parse("sdiff foo bar"),
            res_ok(ParamsSDiff {
                executable: Executable::SDiff,
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
        );

        // file_1 only
        assert_eq!(
            parse("sdiff foo"),
            res_ok(ParamsSDiff {
                executable: Executable::SDiff,
                from: os("foo"),
                to: os("-"),
                ..Default::default()
            }),
        );

        // double dash without operand
        assert_eq!(
            parse("sdiff foo -- --help"),
            res_ok(ParamsSDiff {
                executable: Executable::SDiff,
                from: os("foo"),
                to: os("--help"),
                ..Default::default()
            }),
        );

        // Err: no arguments
        let msg = "missing operand after 'sdiff'";
        match parse("sdiff") {
            Ok(_) => assert!(false, "Should not be ok!"),
            Err(e) => assert!(
                e.to_string().contains(msg),
                "error must contain: \"{msg}\"\nactual error: \"{e}\""
            ),
        }

        // Err: too many operands
        let msg = "extra operand 'should-not-be-here'";
        match parse("sdiff foo bar should-not-be-here") {
            Ok(_) => assert!(false, "Should not be ok!"),
            Err(e) => assert!(
                e.to_string().contains(msg),
                "error must contain: \"{msg}\"\nactual error: \"{e}\""
            ),
        }
    }

    #[test]
    fn execution_modes() {
        // Test all options
        // Disable feature "feat_check_not_yet_implemented"
        // I^A is at the end of the single options, forcing '^A' as argument for 'I'.
        // --wi is abbreviated and uses equal sign
        // diff-program uses next arg
        // -O uses next arg
        let params = ParamsSDiff {
            executable: Executable::SDiff,
            from: os("foo"),
            to: os("bar"),
            diff_program: Some("prg".to_string()),
            expand_tabs: true,
            help: false,
            ignore_all_space: true,
            ignore_blank_lines: true,
            ignore_case: true,
            ignore_matching_lines: Some("^A".to_string()),
            ignore_space_change: true,
            ignore_tab_expansion: true,
            ignore_trailing_space: true,
            left_column: true,
            minimal: true,
            output: Some("out".to_string()),
            speed_large_files: true,
            strip_trailing_cr: true,
            suppress_common_lines: true,
            tabsize: 2,
            text: true,
            version: false,
            width: 150,
        };
        let r = parse(
            "sdiff foo bar -iEZbWBalstdHI^A --wi=150 --diff-program prg -o out --strip --tab=2",
        );
        match &r {
            Ok(_) => assert_eq!(r, res_ok(params.clone())),
            Err(e) => match e {
                ParseError::NotYetImplemented(_) => {}
                _ => assert_eq!(r, res_ok(params.clone())),
            },
        }

        // negative value
        // let msg = "invalid argument '-2' for '--tabsize'";
        let msg = "invalid --tabsize value '-2'";
        let r = parse("sdiff foo bar --tab=-2");
        match r {
            Ok(_) => assert!(false, "Should not be Ok."),
            Err(e) => assert!(
                e.to_string().contains(msg),
                "Must contain: {msg}\nactual: {e}"
            ),
        }
    }
}
