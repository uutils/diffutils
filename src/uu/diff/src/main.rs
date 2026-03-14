// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// TODO implement macro and internationalization
// uucore::bin!(uu_diff);

use std::io::Write;

pub fn main() {
    let code = uu_diff::uumain(uucore::args_os());
    if let Err(e) = std::io::stdout().flush() {
        {
            eprint!("Error flushing stdout: {e}");
        };
    }
    std::process::exit(code);
}
