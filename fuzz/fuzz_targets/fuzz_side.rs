#![no_main]
#[macro_use]
extern crate libfuzzer_sys;

use diffutilslib::side_diff;

use diffutilslib::params::Params;
use std::fs::{self, File};
use std::io::Write;

fuzz_target!(|x: (Vec<u8>, Vec<u8>, u16, u16, bool)| {
    let (original, new, width, tabsize, expand) = x;

    let params = Params {
        width: width as usize,
        tabsize: tabsize as usize,
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
