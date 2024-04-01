#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
use diffutilslib::ed_diff;
use diffutilslib::ed_diff::DiffError;
use diffutilslib::params::Params;
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;

fn diff_w(expected: &[u8], actual: &[u8], filename: &str) -> Result<Vec<u8>, DiffError> {
    let mut output = ed_diff::diff(expected, actual, &Params::default())?;
    writeln!(&mut output, "w {filename}").unwrap();
    Ok(output)
}

fuzz_target!(|x: (Vec<u8>, Vec<u8>)| {
    let (mut from, mut to) = x;
    from.push(b'\n');
    to.push(b'\n');
    if let Ok(s) = String::from_utf8(from.clone()) {
        if !s.is_ascii() {
            return;
        }
        if s.find(|x| x < ' ' && x != '\n').is_some() {
            return;
        }
    } else {
        return;
    }
    if let Ok(s) = String::from_utf8(to.clone()) {
        if !s.is_ascii() {
            return;
        }
        if s.find(|x| x < ' ' && x != '\n').is_some() {
            return;
        }
    } else {
        return;
    }
    let diff = diff_w(&from, &to, "target/fuzz.file").unwrap();
    File::create("target/fuzz.file.original")
        .unwrap()
        .write_all(&from)
        .unwrap();
    File::create("target/fuzz.file.expected")
        .unwrap()
        .write_all(&to)
        .unwrap();
    File::create("target/fuzz.file")
        .unwrap()
        .write_all(&from)
        .unwrap();
    File::create("target/fuzz.ed")
        .unwrap()
        .write_all(&diff)
        .unwrap();
    let output = Command::new("ed")
        .arg("target/fuzz.file")
        .stdin(File::open("target/fuzz.ed").unwrap())
        .output()
        .unwrap();
    if !output.status.success() {
        panic!(
            "STDOUT:\n{}\nSTDERR:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let result = fs::read("target/fuzz.file").unwrap();
    if result != to {
        panic!(
            "STDOUT:\n{}\nSTDERR:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
});
