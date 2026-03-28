// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

//! Common errors for all diffutils utilities.

use std::ffi::OsString;

use uucore::parser::parse_size::ParseSizeError;

use crate::{error::UError, translate};

/// Contains common Core/DiffUtils errors and their text messages.
///
/// Returns exit code 2, if a different exit code is required,
/// use [UtilsErrorCode]
///
/// A typical way to return an std::io:Error as
/// Box<dyn UError> (from [crate::error::UResult]) is:
/// Err => {
///     let io = error.map_err_context(|| path.to_string_lossy().to_string());
///     return Err(UtilsError::Io(io).into());
/// }
// Clone and PartialEq cannot be derived for Box<dyn Error>.
#[derive(Debug)]
pub enum UtilsError {
    /// When a util does not handle directories (e.g. cmp).
    ///
    /// Param: wrong operand (dir name)
    DirectoryNotAllowed(OsString),

    /// Generic IO error, Display handled by [crate::error::UIoError]
    Io(Box<dyn UError>),
    IoDouble(Box<dyn UError>, Box<dyn UError>),
}

impl std::error::Error for UtilsError {}

impl UError for UtilsError {
    fn code(&self) -> i32 {
        2
    }
}

impl From<std::io::Error> for UtilsError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.into())
    }
}

impl std::fmt::Display for UtilsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::DirectoryNotAllowed(dir) => {
                translate!("error-is-a-directory", "file" => dir.to_string_lossy())
            }
            Self::Io(e) => {
                // dbg!("Io");
                return e.fmt(f);
            }
            Self::IoDouble(e1, e2) => {
                format!("{e1}\n{}: {e2}", uucore::util_name())
            }
        };

        write!(f, "{msg}")
    }
}

/// Like [UtilsError] with the option to specify the exit code.
///
/// A typical way to return an std::io:Error as
/// Box<dyn UError> (from [crate::error::UResult]) is:
/// Err => {
///     let io = error.map_err_context(|| path.to_string_lossy().to_string());
///     return Err(UtilsErrorCode::new(UtilsError::Io(io), 4).into());
/// }
#[derive(Debug)]
pub struct UtilsErrorCode {
    pub utils_error: UtilsError,
    pub code: i32,
}

impl UtilsErrorCode {
    pub fn new(utils_error: UtilsError, code: i32) -> Self {
        Self { utils_error, code }
    }
}

impl std::error::Error for UtilsErrorCode {}

impl UError for UtilsErrorCode {
    fn code(&self) -> i32 {
        self.code
    }
}

impl std::fmt::Display for UtilsErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.utils_error.fmt(f)
    }
}

/// Contains all parser errors and their text messages.
///
/// All errors can be output easily using the normal Display functionality.
/// To format the error message for the typical diffutils output, use [format_error_text].
#[derive(Debug, PartialEq, Eq)]
pub enum UParseError {
    /// (Option, value, error)
    ParseSizeError(&'static str, String, ParseSizeError),

    /// (Format options)
    ConflictingOutputStyle(String, String),

    /// Having more operands than the four allowed (file_1, file_2, ign_1, ign_2)
    ///
    /// Params: (wrong operand)
    ExtraOperand(OsString),

    InvalidContextLength(String),
    InvalidUnifiedLength(String),

    /// Operand missing, e.g. diff without files
    MissingOperand(String),

    /// Two options cannot be used together, e.g. cmp --silent and --verbose (output).
    OptionsIncompatible(&'static str, &'static str),

    /// Error message for options available in GNU, but not yet here
    NotYetImplemented(&'static str),
}

impl std::error::Error for UParseError {}

impl UError for UParseError {
    fn code(&self) -> i32 {
        2
    }

    fn usage(&self) -> bool {
        // TODO should not returns full path on try --help message
        // Try '/home/gunnar/SynologyDrive/Development/diffutils_fork/target/debug/cmp --help' for more information.
        true
    }
}

impl std::fmt::Display for UParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::ParseSizeError(option, value, e) => match e {
                ParseSizeError::InvalidSuffix(_) => {
                    translate!(
                        "parse-error-invalid-value-unit",
                        "option" => option,
                        "value" => value
                    )
                }
                ParseSizeError::ParseFailure(_) => {
                    translate!(
                        "parse-error-invalid-value",
                        "option" => option,
                        "value" => value
                    )
                }
                ParseSizeError::SizeTooBig(_) => {
                    translate!(
                        "parse-error-invalid-value-overflow",
                        "option" => option,
                        "value" => value
                    )
                }
                ParseSizeError::PhysicalMem(_value) => e.to_string(),
            },

            Self::ConflictingOutputStyle(opt_1, opt_2) => {
                translate!("parse-error-conflicting-output-options", "opt1" => opt_1, "opt2" => opt_2)
            }
            Self::ExtraOperand(extra_operand) => {
                translate!("parse-error-extra-operand", "operand" => extra_operand.to_string_lossy())
            }
            Self::InvalidContextLength(value) => {
                translate!("parse-error-invalid-context-length", "value" => value)
            }
            Self::InvalidUnifiedLength(value) => {
                translate!("parse-error-invalid-unified-length", "value" => value)
            }
            Self::MissingOperand(after) => {
                translate!("parse-error-missing-operand", "after" => after)
            }
            Self::OptionsIncompatible(option_1, option_2) => translate!(
                "parse-error-incompatible-options",
                "opt1" => option_1,
                "opt2" => option_2,
            ),
            Self::NotYetImplemented(s) => {
                translate!("parse-error-not-yet-implemented", "option" => s)
            }
        };
        write!(f, "{msg}")
    }
}
