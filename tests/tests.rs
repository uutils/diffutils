// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::env;

pub const TESTS_BINARY: &str = env!("CARGO_BIN_EXE_diffutils");

// uutests runs the binary named by UUTESTS_BINARY_PATH, with the utility name
// as its first argument.
#[ctor::ctor(unsafe)]
fn init() {
    env::set_var("UUTESTS_BINARY_PATH", TESTS_BINARY);
}

#[path = "test_common.rs"]
mod test_common;

#[path = "by-util/test_cmp.rs"]
mod test_cmp;

#[path = "by-util/test_diff.rs"]
mod test_diff;
