#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
use diffutilslib::cmp;

use libfuzzer_sys::Corpus;
use std::ffi::OsString;

fn os(s: &str) -> OsString {
    OsString::from(s)
}

fuzz_target!(|x: Vec<OsString>| -> Corpus {
    if x.len() > 6 {
        // Make sure we try to parse an option when we get longer args. x[0] will be
        // the executable name.
        if ![os("-l"), os("-b"), os("-s"), os("-n"), os("-i")].contains(&x[1]) {
            return Corpus::Reject;
        }
    }
    let _ = cmp::parse_params(x.into_iter().peekable());
    Corpus::Keep
});
