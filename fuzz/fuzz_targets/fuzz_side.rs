#![no_main]
#[macro_use]
extern crate libfuzzer_sys;

use diffutilslib::side_diff;

use diffutilslib::params::Params;
use std::fs::{self, File};
use std::io::Write;

// We can't fuzz with width equals to usize, otherwise we
// would could have 2^64 - 1 of padding columns, which means
// exabytes nescessary for this. u32 also doesn't have a
// great perfomance here, with almost 537 MB being nescessary
// and 57 seconds of execution.
fuzz_target!(|x: (Vec<u8>, Vec<u8>, u16, usize, bool)| {
    let (original, new, width, tabsize, expand) = x;

    let params = Params {
        width: width as usize,
        tabsize,
        expand_tabs: expand,
        ..Default::default()
    };
    fs::create_dir_all("target").unwrap();
    let mut output_buf = vec![];
    side_diff::diff(&original, &new, &mut output_buf, &params);
    File::create("target/fuzz.file.original")
        .unwrap()
        .write_all(&original)
        .unwrap();
    File::create("target/fuzz.file.new")
        .unwrap()
        .write_all(&new)
        .unwrap();
    File::create("target/fuzz.file")
        .unwrap()
        .write_all(&original)
        .unwrap();
    File::create("target/fuzz.diff")
        .unwrap()
        .write_all(&output_buf)
        .unwrap();
});
