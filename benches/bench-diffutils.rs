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
const FILE_SIZE_KILO_BYTES: [u64; 4] = [100, 1 * MB, 10 * MB, 25 * MB];
// const FILE_SIZE_KILO_BYTES: [u64; 3] = [100, 1 * MB, 5 * MB];
// Empty String to use TempDir (files will be removed after test) or specify dir to keep generated files
const TEMP_DIR: &str = "";
const NUM_DIFF: u64 = 4;
// just for FILE_SIZE_KILO_BYTES
const MB: u64 = 1_000;
const CHANGE_CHAR: u8 = b'#';

#[cfg(not(feature = "feat_bench_not_cmp"))]
mod diffutils_cmp {
    use std::hint::black_box;

    use diffutilslib::cmp;
    use divan::Bencher;

    use crate::{binary, prepare::*, FILE_SIZE_KILO_BYTES};

    #[divan::bench(args = FILE_SIZE_KILO_BYTES)]
    fn cmp_compare_files_equal(bencher: Bencher, kb: u64) {
        let (from, to) = get_context().get_test_files_equal(kb);
        let cmd = format!("cmp {from} {to}");
        let opts = str_to_options(&cmd).into_iter().peekable();
        let params = cmp::parse_params(opts).unwrap();

        bencher
            // .with_inputs(|| prepare::cmp_params_identical_testfiles(lines))
            .with_inputs(|| params.clone())
            .bench_refs(|params| black_box(cmp::cmp(&params).unwrap()));
    }

    // bench the actual compare; cmp exits on first difference
    #[divan::bench(args = FILE_SIZE_KILO_BYTES)]
    fn cmp_compare_files_different(bencher: Bencher, bytes: u64) {
        let (from, to) = get_context().get_test_files_different(bytes);
        let cmd = format!("cmp {from} {to} -s");
        let opts = str_to_options(&cmd).into_iter().peekable();
        let params = cmp::parse_params(opts).unwrap();

        bencher
            // .with_inputs(|| prepare::cmp_params_identical_testfiles(lines))
            .with_inputs(|| params.clone())
            .bench_refs(|params| black_box(cmp::cmp(&params).unwrap()));
    }

    // bench original GNU cmp
    #[divan::bench(args = FILE_SIZE_KILO_BYTES)]
    fn cmd_cmp_gnu_equal(bencher: Bencher, bytes: u64) {
        let (from, to) = get_context().get_test_files_equal(bytes);
        let args_str = format!("{from} {to}");
        bencher
            // .with_inputs(|| prepare::cmp_params_identical_testfiles(lines))
            .with_inputs(|| args_str.clone())
            .bench_refs(|cmd_args| binary::bench_binary("cmp", cmd_args));
    }

    // bench the compiled release version
    #[divan::bench(args = FILE_SIZE_KILO_BYTES)]
    fn cmd_cmp_release_equal(bencher: Bencher, bytes: u64) {
        let (from, to) = get_context().get_test_files_equal(bytes);
        let args_str = format!("cmp {from} {to}");

        bencher
            // .with_inputs(|| prepare::cmp_params_identical_testfiles(lines))
            .with_inputs(|| args_str.clone())
            .bench_refs(|cmd_args| binary::bench_binary("target/release/diffutils", cmd_args));
    }
}

#[cfg(not(feature = "feat_bench_not_diff"))]
mod diffutils_diff {
    // use std::hint::black_box;

    use crate::{binary, prepare::*, FILE_SIZE_KILO_BYTES};
    // use diffutilslib::params;
    use divan::Bencher;

    // bench the actual compare
    // TODO diff does not have a diff function
    //     #[divan::bench(args = [100_000,10_000])]
    //     fn diff_compare_files(bencher: Bencher, bytes: u64) {
    //         let (from, to) = gen_testfiles(lines, 0, "id");
    //         let cmd = format!("cmp {from} {to}");
    //         let opts = str_to_options(&cmd).into_iter().peekable();
    //         let params = params::parse_params(opts).unwrap();
    //
    //         bencher
    //             // .with_inputs(|| prepare::cmp_params_identical_testfiles(lines))
    //             .with_inputs(|| params.clone())
    //             .bench_refs(|params| diff::diff(&params).unwrap());
    //     }

    // bench original GNU diff
    #[divan::bench(args = FILE_SIZE_KILO_BYTES)]
    fn cmd_diff_gnu_equal(bencher: Bencher, bytes: u64) {
        let (from, to) = get_context().get_test_files_equal(bytes);
        let args_str = format!("{from} {to}");
        bencher
            // .with_inputs(|| prepare::cmp_params_identical_testfiles(lines))
            .with_inputs(|| args_str.clone())
            .bench_refs(|cmd_args| binary::bench_binary("diff", cmd_args));
    }

    // bench the compiled release version
    #[divan::bench(args = FILE_SIZE_KILO_BYTES)]
    fn cmd_diff_release_equal(bencher: Bencher, bytes: u64) {
        let (from, to) = get_context().get_test_files_equal(bytes);
        let args_str = format!("diff {from} {to}");

        bencher
            // .with_inputs(|| prepare::cmp_params_identical_testfiles(lines))
            .with_inputs(|| args_str.clone())
            .bench_refs(|cmd_args| binary::bench_binary("target/release/diffutils", cmd_args));
    }
}

mod parser {
    use std::hint::black_box;

    use diffutilslib::{cmp, params};
    use divan::Bencher;

    use crate::prepare::str_to_options;

    // bench the time it takes to parse the command line arguments
    #[divan::bench]
    fn cmp_parser(bencher: Bencher) {
        let cmd = "cmd file_1.txt file_2.txt -bl n10M --ignore-initial=100KiB:1MiB";
        let args = str_to_options(&cmd).into_iter().peekable();
        bencher
            .with_inputs(|| args.clone())
            .bench_values(|data| black_box(cmp::parse_params(data)));
    }

    // // test the impact on the benchmark if not converting the cmd to Vec<OsString> (doubles for parse)
    // #[divan::bench]
    // fn cmp_parser_no_prepare() {
    //     let cmd = "cmd file_1.txt file_2.txt -bl n10M --ignore-initial=100KiB:1MiB";
    //     let args = str_to_options(&cmd).into_iter().peekable();
    //     let _ = cmp::parse_params(args);
    // }

    // bench the time it takes to parse the command line arguments
    #[divan::bench]
    fn diff_parser(bencher: Bencher) {
        let cmd = "diff file_1.txt file_2.txt -s --brief --expand-tabs --width=100";
        let args = str_to_options(&cmd).into_iter().peekable();
        bencher
            .with_inputs(|| args.clone())
            .bench_values(|data| black_box(params::parse_params(data)));
    }
}

mod prepare {
    use std::{
        ffi::OsString,
        fs::{self, File},
        io::{BufWriter, Write},
        path::Path,
        sync::OnceLock,
    };

    use rand::RngExt;
    use tempfile::TempDir;

    use crate::{CHANGE_CHAR, FILE_SIZE_KILO_BYTES, NUM_DIFF, TEMP_DIR};

    // file lines and .txt will be added
    const FROM_FILE: &str = "from_file";
    const TO_FILE: &str = "to_file";
    const LINE_LENGTH: usize = 60;

    /// Contains test data (file names) which only needs to be created once.
    #[derive(Debug, Default)]
    pub struct BenchContext {
        pub tmp_dir: Option<TempDir>,
        pub dir: String,
        pub files_equal: Vec<(String, String)>,
        pub files_different: Vec<(String, String)>,
    }

    impl BenchContext {
        pub fn get_path(&self) -> &Path {
            match &self.tmp_dir {
                Some(tmp) => tmp.path(),
                None => Path::new(&self.dir),
            }
        }

        pub fn get_test_files_equal(&self, kb: u64) -> &(String, String) {
            let p = FILE_SIZE_KILO_BYTES.iter().position(|f| *f == kb).unwrap();
            &self.files_equal[p]
        }

        #[allow(unused)]
        pub fn get_test_files_different(&self, kb: u64) -> &(String, String) {
            let p = FILE_SIZE_KILO_BYTES.iter().position(|f| *f == kb).unwrap();
            &self.files_different[p]
        }
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
                    fs::create_dir_all(path).expect("Path {path} could not be created");
                }
                ctx.dir = TEMP_DIR.to_string();
            };

            // generate test bytes
            for kb in FILE_SIZE_KILO_BYTES {
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

    pub fn str_to_options(opt: &str) -> Vec<OsString> {
        let s: Vec<OsString> = opt
            .split(" ")
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(|s| OsString::from(s))
            .collect();

        s
    }

    /// Generates two test files for comparison with <bytes> size.
    ///
    /// Each line consists of 10 words with 5 letters, giving a line length of 60 bytes.
    /// If num_differences is set, '#' will be inserted between the first two words of a line,
    /// evenly spaced in the file. 1 will add the change in the last line, so the comparison takes longest.
    fn generate_test_files_bytes(
        dir: &Path,
        bytes: u64,
        num_differences: u64,
        id: &str,
    ) -> std::io::Result<(String, String)> {
        let id = if id.is_empty() {
            "".to_string()
        } else {
            format!("{id}_")
        };
        let f1 = format!("{id}{FROM_FILE}_{bytes}.txt");
        let f2 = format!("{id}{TO_FILE}_{bytes}.txt");
        let from_path = dir.join(f1);
        let to_path = dir.join(f2);

        generate_file_bytes(&from_path, &to_path, bytes, num_differences)?;

        Ok((
            from_path.to_string_lossy().to_string(),
            to_path.to_string_lossy().to_string(),
        ))
    }

    fn generate_file_bytes(
        from_name: &Path,
        to_name: &Path,
        bytes: u64,
        num_differences: u64,
    ) -> std::io::Result<()> {
        let file_from = File::create(from_name)?;
        let file_to = File::create(to_name)?;
        // for int division, lines will be smaller than requested bytes
        let n_lines = bytes / LINE_LENGTH as u64;
        let change_every_n_lines = if num_differences == 0 {
            0
        } else {
            let c = n_lines / num_differences;
            if c == 0 {
                1
            } else {
                c
            }
        };
        // Use a larger 128KB buffer for massive files
        let mut writer_from = BufWriter::with_capacity(128 * 1024, file_from);
        let mut writer_to = BufWriter::with_capacity(128 * 1024, file_to);
        let mut rng = rand::rng();

        // Each line: (5 chars * 10 words) + 9 spaces + 1 newline = 60 bytes
        let mut line_buffer = [b' '; 60];
        line_buffer[59] = b'\n'; // Set the newline once at the end

        for i in (0..n_lines).rev() {
            // Fill only the letter positions, skipping spaces and the newline
            for word_idx in 0..10 {
                let start = word_idx * 6; // Each word + space block is 6 bytes
                for i in 0..5 {
                    line_buffer[start + i] = rng.random_range(b'a'..b'z' + 1);
                }
            }

            // Write the raw bytes directly to both files
            writer_from.write_all(&line_buffer)?;
            // make changes in the file
            if num_differences == 0 {
                writer_to.write_all(&line_buffer)?;
            } else {
                if i % change_every_n_lines == 0 && n_lines - i > 2 {
                    line_buffer[5] = CHANGE_CHAR;
                }
                writer_to.write_all(&line_buffer)?;
                line_buffer[5] = b' ';
            }
        }

        // create last line
        let missing = (bytes - n_lines as u64 * LINE_LENGTH as u64) as usize;
        if missing > 0 {
            for word_idx in 0..10 {
                let start = word_idx * 6; // Each word + space block is 6 bytes
                for i in 0..5 {
                    line_buffer[start + i] = rng.random_range(b'a'..b'z' + 1);
                }
            }
            line_buffer[missing - 1] = b'\n';
            writer_from.write_all(&line_buffer[0..missing])?;
            writer_to.write_all(&line_buffer[0..missing])?;
        }

        writer_from.flush()?;
        writer_to.flush()?;

        Ok(())
    }
}

mod binary {
    use std::process::Command;

    use crate::prepare::str_to_options;

    pub fn bench_binary(program: &str, cmd_args: &str) -> std::process::ExitStatus {
        let args = str_to_options(cmd_args);
        Command::new(program)
            .args(args)
            .status()
            .expect("Failed to execute binary")
    }
}

fn main() {
    // Run registered benchmarks.
    divan::main();
}
