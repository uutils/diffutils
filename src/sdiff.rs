// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

//! This module holds the core compare logic of sdiff.
pub mod params_sdiff;

use std::{
    env::ArgsOs,
    ffi::OsString,
    fs,
    io::{self, stdout, Read, Write},
    iter::Peekable,
    process::ExitCode,
};

use crate::{
    arg_parser::{
        add_copyright, format_error_text, get_version_text, Executable, ParseError,
        TEXT_HELP_FOOTER,
    },
    sdiff::params_sdiff::{ParamsSDiff, SDiffParseOk},
    side_diff, utils,
};

// This contains the hard coded 'sdiff'. If required this needs to be replaced with the executable.
pub const TEXT_HELP: &str = const_format::concatcp!(
    r#"
    sdiff is a tool which allows to compare two text files for differences.
    It outputs the differences in a side-by-side view.
    Use 'diff' for a row-by-row view.
    Use 'cmp' to compare binary files.
    
    Usage: sdiff [OPTIONS] [FILE]...
    If a FILE is '-', read operating system's standard input.

    Options:
    -o, --output=FILE                  operate interactively while sending output to FILE
        --diff-program=PROGRAM         use PROGRAM to compare files
    -a, --text                         treat all files as text
    -H, --speed-large-files            assume large files with many scattered small changes
    -d, --minimal                      try to find a smaller set of changes
        
    -i, --ignore-case                  do not distinguish between upper- and lower-case letters
    -E, --ignore-tab-expansion         ignore changes due to tab expansion
    -Z, --ignore-trailing-space        ignore white space at line end
    -b, --ignore-space-change          ignore changes in the amount of white space
    -W, --ignore-all-space             ignore all white space
    -B, --ignore-blank-lines           ignore changes whose lines are all blank
    -I, --ignore-matching-lines=REGEX  ignore changes all whose lines match REGEX expression
        --strip-trailing-cr            strip trailing carriage return on input

    -s, --suppress-common-lines        do not output common lines
    -l, --left-column                  output only the left column of common lines
    -t, --expand-tabs                  expand tabs to spaces in output
        --tabsize=NUM                  tab stops at every NUM (default 8) print columns
    -w, --width=NUM                    limit the print width to NUM print columns (default 130) 

    -h  --help                         display this help and exit
    -v, --version                      output version information and exit

    Exit status is 0 if inputs are identical, 1 if different, 2 in error case.
    "#,
    TEXT_HELP_FOOTER
);

/// Entry into sdiff.
///
/// Param options, e.g. 'sdiff file1.txt file2.txt -bd n2000kB'. \
/// sdiff options as documented in the GNU manual.
///
/// Ends program with Exit Status:
/// * 0 if inputs are identical
/// * 1 if inputs are different
/// * 2 in error case
pub fn main(mut args: Peekable<ArgsOs>) -> ExitCode {
    let Some(executable) = Executable::from_args_os(&mut args, false) else {
        eprintln!("Expected utility name as first argument, got nothing.");
        return ExitCode::FAILURE;
    };
    match sdiff(args) {
        Ok(res) => match res {
            SDiffOk::Different => ExitCode::FAILURE,
            SDiffOk::Equal => ExitCode::SUCCESS,
            SDiffOk::Help => {
                println!("{}", add_copyright(TEXT_HELP));
                ExitCode::SUCCESS
            }
            SDiffOk::Version => {
                println!("{}", get_version_text(&executable));
                ExitCode::SUCCESS
            }
        },
        Err(e) => {
            let msg = match e {
                SDiffError::ReadFileErrors(_, _) => {
                    format!("{e}")
                }
                _ => format_error_text(&executable, &e),
            };
            // let msg = format_error_text(&executable, &e);
            eprintln!("{msg}");
            ExitCode::from(2)
        }
    }
}

/// This is the full sdiff call.
///
/// The first arg needs to be the executable, then the operands and options.
pub fn sdiff<I: Iterator<Item = OsString>>(mut args: Peekable<I>) -> Result<SDiffOk, SDiffError> {
    let Some(executable) = Executable::from_args_os(&mut args, true) else {
        return Err(ParseError::NoExecutable.into());
    };
    // read params
    let params = match ParamsSDiff::parse_params(&executable, args)? {
        SDiffParseOk::Params(p) => p,
        SDiffParseOk::Help => return Ok(SDiffOk::Help),
        SDiffParseOk::Version => return Ok(SDiffOk::Version),
    };
    // dbg!("{params:?}");

    // compare files
    sdiff_compare(&params)
}

/// This is the main function to compare the files. \
///
/// TODO sdiff is missing a number of options, currently implemented:
/// * expand_tabs
/// * tabsize
/// * width
/// * The output format does not match GNU sdiff
pub fn sdiff_compare(params: &ParamsSDiff) -> Result<SDiffOk, SDiffError> {
    if utils::is_same_file(&params.from, &params.to) {
        return Ok(SDiffOk::Equal);
    }

    let (from_content, to_content) = match read_both_files(&params.from, &params.to) {
        Ok(files) => files,
        Err(errors) => {
            let mut vs = Vec::new();
            for (file, e) in errors {
                let s = utils::format_failure_to_read_input_file(
                    &params.executable.to_os_string(),
                    &file,
                    &e,
                );
                vs.push(s);
            }
            return Err(SDiffError::ReadFileErrors(
                params.executable.clone(),
                vs.to_vec(),
            ));
        }
    };

    // run diff
    let mut output = stdout().lock();
    let result = side_diff::diff(&from_content, &to_content, &mut output, &params.into());

    match std::io::stdout().write_all(&result) {
        Ok(_) => {
            if result.is_empty() {
                Ok(SDiffOk::Equal)
            } else {
                Ok(SDiffOk::Different)
            }
        }
        Err(e) => Err(SDiffError::OutputError(e.to_string())),
    }
}

/// Helper function to read a file fully into memory.
// While this could be in utils, the functionality is limited to files which fit into memory.
// TODO will not work for large files, need buffered approach.
pub fn read_file_contents(filepath: &OsString) -> io::Result<Vec<u8>> {
    if filepath == "-" {
        let mut content = Vec::new();
        io::stdin().read_to_end(&mut content).and(Ok(content))
    } else {
        fs::read(filepath)
    }
}

/// Reads both files and returns the files or a list of errors, as both files can produce a separate error.
pub type ResultReadBothFiles = Result<(Vec<u8>, Vec<u8>), Vec<(OsString, io::Error)>>;
/// Reads both files and returns the files or a list of errors, as both files can produce a separate error.
pub fn read_both_files(from: &OsString, to: &OsString) -> ResultReadBothFiles {
    let mut read_errors = Vec::new();
    let from_content = match read_file_contents(from).map_err(|e| (from.clone(), e)) {
        Ok(r) => r,
        Err(e) => {
            read_errors.push(e);
            Vec::new()
        }
    };
    let to_content = match read_file_contents(to).map_err(|e| (to.clone(), e)) {
        Ok(r) => r,
        Err(e) => {
            read_errors.push(e);
            Vec::new()
        }
    };

    if read_errors.is_empty() {
        Ok((from_content, to_content))
    } else {
        Err(read_errors)
    }
}

/// The Ok result of sdiff.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SDiffOk {
    Different,
    Equal,
    Help,
    Version,
}

/// Errors for sdiff.
///
/// To centralize error messages and make it easier to use in a lib.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum SDiffError {
    // parse errors
    ParseError(ParseError),

    // compare errors
    OutputError(String),
    ReadFileErrors(Executable, Vec<String>),
}

impl std::error::Error for SDiffError {}

impl From<ParseError> for SDiffError {
    fn from(e: ParseError) -> Self {
        Self::ParseError(e)
    }
}

impl std::fmt::Display for SDiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SDiffError::ParseError(e) => write!(f, "{e}"),
            SDiffError::OutputError(msg) => write!(f, "{msg}"),
            SDiffError::ReadFileErrors(_exe, vec_err) => write!(f, "{}", vec_err.join("\n")),
        }
    }
}
