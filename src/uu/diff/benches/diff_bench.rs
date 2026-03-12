// #![allow(unused)]
// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

//! Benches for all utils in diffutils.
//!
//! There is a file generator included to create files of different sizes for comparison. \
//! Set the TEMP_DIR const to keep the files. df_to_ files have small changes in them, search for '#'. \
//! File generation up to 1 GB is really fast, Benchmarking above 100 MB takes very long.

/// Generate test files with these sizes in KB.
const FILE_SIZES_IN_KILO_BYTES: [u64; 4] = [100, 1 * MB, 10 * MB, 25 * MB];
const NUM_DIFF: u64 = 4;
// Empty String to use TempDir (files will be removed after test) or specify dir to keep generated files
const TEMP_DIR: &str = "";
// just for FILE_SIZE_KILO_BYTES
const MB: u64 = 1_000;

use divan::Bencher;
use std::{path::Path, sync::OnceLock};
use tempfile::TempDir;
use uudiff::benchmark::{
    binary,
    prepare_bench::{generate_test_files_bytes, BenchContext},
    str_to_args,
};

// bench the time it takes to parse the command line arguments
#[divan::bench]
fn diff_parser(bencher: Bencher) {
    let cmd = "diff file_1.txt file_2.txt -s --brief --expand-tabs --width=100";
    let args = str_to_args(&cmd).into_iter().peekable();
    bencher
        .with_inputs(|| args.clone())
        .bench_values(|data| uu_diff::params::parse_params(data));
}

// bench the actual compare
// bench equal, full file read
#[divan::bench(args = FILE_SIZES_IN_KILO_BYTES)]
fn diff_compare_files_equal(bencher: Bencher, kb: u64) {
    let fp = get_context().get_files_equal_kb(kb).unwrap();
    let cmd = format!("diff {} {}", fp.from, fp.to);
    let args = str_to_args(&cmd).into_iter();

    bencher
        // .with_inputs(|| prepare::diff_params_identical_testfiles(lines))
        .with_inputs(|| args.clone())
        .bench_refs(|params| uu_diff::uumain(params.peekable()));
}

// bench original GNU diff
#[divan::bench(args = FILE_SIZES_IN_KILO_BYTES)]
fn cmd_diff_gnu_equal(bencher: Bencher, kb: u64) {
    let fp = get_context().get_files_equal_kb(kb).unwrap();
    let args_str = format!("{} {}", fp.from, fp.to);
    bencher
        // .with_inputs(|| prepare::cmp_params_identical_testfiles(lines))
        .with_inputs(|| args_str.clone())
        .bench_refs(|cmd_args| binary::bench_binary("diff", cmd_args));
}

// bench the compiled release version
#[divan::bench(args = FILE_SIZES_IN_KILO_BYTES)]
fn cmd_diff_release_equal(bencher: Bencher, kb: u64) {
    // search for src, then shorten path
    let dir = std::env::current_dir().unwrap();
    let path = dir.to_string_lossy();
    let path = path.trim_end_matches("src/uu/diff");
    let prg = path.to_string() + "target/release/diffutils";

    let fp = get_context().get_files_equal_kb(kb).unwrap();
    let args_str = format!("diff {} {}", fp.from, fp.to);

    bencher
        // .with_inputs(|| prepare::cmp_params_identical_testfiles(lines))
        .with_inputs(|| args_str.clone())
        .bench_refs(|cmd_args| binary::bench_binary(&prg, cmd_args));
}

// Since each bench function is separate in Divan it is more difficult to dynamically create test data.
// This keeps the TempDir alive until the program exits and generates the files only once.
static SHARED_CONTEXT: OnceLock<BenchContext> = OnceLock::new();
/// Creates the test files once and provides them to all tests.
pub fn get_context() -> &'static BenchContext {
    SHARED_CONTEXT.get_or_init(|| {
        let mut ctx = BenchContext::default();
        if TEMP_DIR.is_empty() {
            let tmp_dir = TempDir::new().expect("Failed to create temp dir");
            ctx.tmp_dir = Some(tmp_dir);
        } else {
            // uses current directory, the generated files are kept
            let path = Path::new(TEMP_DIR);
            if !path.exists() {
                std::fs::create_dir_all(path).expect("Path {path} could not be created");
            }
            ctx.dir = TEMP_DIR.to_string();
        };

        // generate test bytes
        for kb in FILE_SIZES_IN_KILO_BYTES {
            let f = generate_test_files_bytes(ctx.get_path(), kb * 1000, 0, "eq")
                .expect("generate_test_files failed");
            ctx.files_equal.push(f);
            let f = generate_test_files_bytes(ctx.get_path(), kb * 1000, NUM_DIFF, "df")
                .expect("generate_test_files failed");
            ctx.files_different.push(f);
        }

        ctx
    })
}

fn main() {
    // Run registered benchmarks.
    divan::main();
}
