// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::env;

pub const TESTS_BINARY: &str = env!("CARGO_BIN_EXE_diffutils");

// Use the ctor attribute to run this function before any tests
#[ctor::ctor]
fn init() {
    unsafe {
        // Necessary for uutests to be able to find the binary
        env::set_var("UUTESTS_BINARY_PATH", TESTS_BINARY);
    }
}

#[cfg(feature = "cmp")]
#[path = "by-util/test_cmp.rs"]
mod test_cmp;

// #[cfg(feature = "diff")]
// #[path = "by-util/test_diff.rs"]
// mod test_diff;
