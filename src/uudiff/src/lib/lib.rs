//##  internal modules
mod features; // feature-gated code modules
mod macros; // crate macros (macro_rules-type; exported to `crate::...`)
mod mods; // core cross-platform modules

pub use crate::mods::utils;

// * cross-platform modules
pub use crate::mods::clap_localization;
pub use crate::mods::error;
pub use crate::mods::locale;

// * feature-gated modules
// #[cfg(feature = "benchmark")]
pub use crate::features::benchmark;

/// Execute utility code for `util`.
///
/// This macro expands to a main function that invokes the `uumain` function in `util`
/// Exits with code returned by `uumain`.
#[macro_export]
macro_rules! bin {
    ($util:ident) => {
        pub fn main() {
            use std::io::Write;
            use uudiff::locale;

            // Preserve inherited SIGPIPE settings (e.g., from env --default-signal=PIPE)
            uucore::panic::preserve_inherited_sigpipe();

            // suppress extraneous error output for SIGPIPE failures/panics
            uucore::panic::mute_sigpipe_panic();
            locale::setup_localization(uucore::get_canonical_util_name(stringify!($util)))
                .unwrap_or_else(|err| {
                    match err {
                        uudiff::locale::LocalizationError::ParseResource {
                            error: err_msg,
                            snippet,
                        } => eprintln!("Localization parse error at {snippet}: {err_msg:?}"),
                        other => eprintln!("Could not init the localization system: {other}"),
                    }
                    std::process::exit(99)
                });

            // execute utility code
            let code = $util::uumain(uucore::args_os());
            // (defensively) flush stdout for utility prior to exit; see <https://github.com/rust-lang/rust/issues/23818>
            if let Err(e) = std::io::stdout().flush() {
                eprintln!("Error flushing stdout: {e}");
            }

            std::process::exit(code);
        }
    };
}

/// Create a localized help template with explicit color control
/// This ensures color detection consistency between clap and our template
pub fn localized_help_template_with_colors(
    util_name: &str,
    colors_enabled: bool,
) -> clap::builder::StyledStr {
    use std::fmt::Write;

    // Ensure localization is initialized for this utility
    let _ = locale::setup_localization(util_name);

    // Get the localized "Usage" label
    let usage_label = crate::locale::translate!("common-usage");

    // Create a styled template
    let mut template = clap::builder::StyledStr::new();

    // Add the basic template parts
    writeln!(template, "{{before-help}}{{about-with-newline}}").unwrap();

    // Add styled usage header (bold + underline like clap's default)
    if colors_enabled {
        write!(
            template,
            "\x1b[1m\x1b[4m{usage_label}:\x1b[0m {{usage}}\n\n"
        )
        .unwrap();
    } else {
        write!(template, "{usage_label}: {{usage}}\n\n").unwrap();
    }

    // Add the rest
    write!(template, "{{all-args}}{{after-help}}").unwrap();

    template
}
