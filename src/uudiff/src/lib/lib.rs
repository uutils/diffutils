mod features; // feature-gated code modules
mod macros; // crate macros (macro_rules-type; exported to `crate::...`)
mod mods; // core cross-platform modules

// pub use crate::mods::arg_parser;
pub use crate::mods::utils;

// * feature-gated modules
// #[cfg(feature = "benchmark")]
pub use crate::features::benchmark;
