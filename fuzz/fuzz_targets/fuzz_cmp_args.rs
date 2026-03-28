#![no_main]
#[macro_use]
extern crate libfuzzer_sys;

use libfuzzer_sys::Corpus;
use std::{convert::TryFrom, ffi::OsString};
use uu_cmp::params_cmp::Params;

fn os(s: &str) -> OsString {
    OsString::from(s)
}

fuzz_target!(|args: Vec<OsString>| -> Corpus {
    if args.len() > 6 {
        // Make sure we try to parse an option when we get longer args. x[0] will be
        // the executable name.
        if ![os("-l"), os("-b"), os("-s"), os("-n"), os("-i")].contains(&args[1]) {
            return Corpus::Reject;
        }
    }
    // not sure what this does, mostly empty args
    // dbg!(&args);
    // let _ = uu_cmp::parse_params(x.into_iter().peekable());
    if let Ok(matches) = uudiff::clap_localization::handle_clap_result_with_exit_code(
        uu_cmp::params_cmp::uu_app(),
        args,
        2,
    ) {
        let _params = Params::try_from(matches);
    }
    Corpus::Keep
});
