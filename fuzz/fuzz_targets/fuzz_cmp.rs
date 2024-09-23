#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
use diffutilslib::cmp::{self, Cmp};

use std::ffi::OsString;
use std::fs::File;
use std::io::Write;

fn os(s: &str) -> OsString {
    OsString::from(s)
}

fuzz_target!(|x: (Vec<u8>, Vec<u8>)| {
    let args = vec!["cmp", "-l", "-b", "target/fuzz.cmp.a", "target/fuzz.cmp.b"]
        .into_iter()
        .map(|s| os(s))
        .peekable();

    let (from, to) = x;

    File::create("target/fuzz.cmp.a")
        .unwrap()
        .write_all(&from)
        .unwrap();

    File::create("target/fuzz.cmp.b")
        .unwrap()
        .write_all(&to)
        .unwrap();

    let params =
        cmp::parse_params(args).unwrap_or_else(|e| panic!("Failed to parse params: {}", e));
    let ret = cmp::cmp(&params);
    if from == to && !matches!(ret, Ok(Cmp::Equal)) {
        panic!(
            "target/fuzz.cmp.a and target/fuzz.cmp.b are equal, but cmp returned {:?}.",
            ret
        );
    } else if from != to && !matches!(ret, Ok(Cmp::Different)) {
        panic!(
            "target/fuzz.cmp.a and target/fuzz.cmp.b are different, but cmp returned {:?}.",
            ret
        );
    } else if ret.is_err() {
        panic!(
            "target/fuzz.cmp.a and target/fuzz.cmp.b caused cmp to error ({:?}).",
            ret
        );
    }
});
