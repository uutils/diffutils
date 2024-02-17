[![Crates.io](https://img.shields.io/crates/v/diffutils.svg)](https://crates.io/crates/diffutils)
[![Discord](https://img.shields.io/badge/discord-join-7289DA.svg?logo=discord&longCache=true&style=flat)](https://discord.gg/wQVJbvJ)
[![License](http://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/uutils/diffutils/blob/main/LICENSE)
[![dependency status](https://deps.rs/repo/github/uutils/diffutils/status.svg)](https://deps.rs/repo/github/uutils/diffutils)

[![CodeCov](https://codecov.io/gh/uutils/diffutils/branch/master/graph/badge.svg)](https://codecov.io/gh/uutils/diffutils)

The goal of this package is to be a dropped in replacement for the [diffutils commands](https://www.gnu.org/software/diffutils/) in Rust.

Based on the incomplete diff generator in https://github.com/rust-lang/rust/blob/master/src/tools/compiletest/src/runtest.rs, and made to be compatible with GNU's diff and patch tools.


## Installation

Ensure you have Rust installed on your system. You can install Rust through [rustup](https://rustup.rs/).

Clone the repository and build the project using Cargo:

```bash
git clone https://github.com/uutils/diffutils.git
cd diffutils
cargo build --release
```

```bash

cat <<EOF >fruits_old.txt
Apple
Banana
Cherry
EOF

cat <<EOF >fruits_new.txt
Apple
Fig
Cherry
EOF

$ cargo run -- -u fruits_old.txt fruits_new.txt
    Finished dev [unoptimized + debuginfo] target(s) in 0.00s
     Running `target/debug/diffutils -u fruits_old.txt fruits_new.txt`
--- fruits_old.txt
+++ fruits_new.txt
@@ -1,3 +1,3 @@
 Apple
-Banana
+Fig
 Cherry

```

## License

diffutils is licensed under the MIT and Apache Licenses - see the `LICENSE-MIT` or `LICENSE-APACHE` files for details
