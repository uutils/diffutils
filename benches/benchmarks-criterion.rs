/// Benchmarks, currently only for cmp
///
/// Provides some general functions, e.g. to create files to compare in different sizes.
///
/// use hyperfine to benchmark against cmp
/// * hyperfine -i "target/release/diffutils cmp from_file_10000000.txt to_file_10000000.txt"  
/// * hyperfine -i "cmp from_file_10000000.txt to_file_10000000.txt"  
///
/// The Rust version seems twice as slow.
use criterion::{criterion_group, criterion_main, Criterion};
// use std::env;
// use std::hint::black_box;
use rand::RngExt;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::Command;
use std::{ffi::OsString, fs::File, time::Duration};

const WARM_UP_TIME_MS: u64 = 500;
#[allow(unused)]
const MEASUREMENT_TIME_MS: u64 = 2000;

// file lines and .txt will be added
const FROM_FILE: &str = "from_file";
const TO_FILE: &str = "to_file";

criterion_group!(
    benches,
    bench_parser,
    bench_cmp // , bench_diff
);
criterion_main!(benches);

// All results are a few microseconds, so negligible.
fn bench_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("Bench parser");

    group.warm_up_time(Duration::from_millis(WARM_UP_TIME_MS));
    // group.measurement_time(Duration::from_millis(MEASUREMENT_TIME_MS));
    // group.sample_size(10);

    group.bench_function("Parse cmp", |b| {
        b.iter(|| {
            cmp_parse_only(
                "cmd file_1.txt file_2.txt -bl --bytes=2048 --ignore-initial=100KiB:1MiB",
            )
        })
    });

    group.bench_function("Parse diff", |b| {
        b.iter(|| diff_parse_only("diff file_1.txt file_2.txt"))
    });
    // group.bench_function("Parse error", |b| {
    //     b.iter(|| parse_single_arg("cmd file_1.txt file_2.txt --something-unknown"))
    // });
    // group.bench_function("Parse help", |b| b.iter(|| parse_single_arg("cmd --help")));

    group.finish();
}

// This is the interesting part.
fn bench_cmp(c: &mut Criterion) {
    let mut group = c.benchmark_group("Bench cmp");
    // uses tmp
    // let dir_path = tempfile::tempdir().unwrap().path();
    // uses current directory, the generated files are kept
    let dir_path = Path::new("");
    // let curr = env::current_dir().unwrap();
    // let dir_path = curr.as_path();
    let num_lines = 100_000;
    // The more differences, the faster cmp returns, as it stops after the first difference is found.
    let num_differences = 1;

    group.warm_up_time(Duration::from_millis(WARM_UP_TIME_MS));
    // group.measurement_time(Duration::from_millis(MEASUREMENT_TIME_MS));
    // group.sample_size(10);

    let (from, to) =
        generate_test_files(num_lines, 0, dir_path).expect("generate_test_files failed");
    let cmd = format!("cmp {from} {to}");
    let opts = str_to_args(&cmd).into_iter().peekable();
    let params = diffutilslib::cmp::parse_params(opts).unwrap();

    group.bench_function(format!("cmp files unchanged, lines: {num_lines}"), |b| {
        b.iter(|| diffutilslib::cmp::cmp(&params).unwrap())
    });

    let (from, to) = generate_test_files(num_lines, num_differences, dir_path)
        .expect("generate_test_files failed");
    let cmd = format!("cmp {from} {to} -s");
    let opts = str_to_args(&cmd).into_iter().peekable();
    let params = diffutilslib::cmp::parse_params(opts).unwrap();

    group.bench_function(format!("cmp files changed, lines: {num_lines}"), |b| {
        b.iter(|| diffutilslib::cmp::cmp(&params).unwrap())
    });

    group.finish();

    // Optional bench by executing the file as cmd
    bench_binary_execution_cmp(c);
}

// // This is the interesting part.
// fn bench_diff(c: &mut Criterion) {
//     let mut group = c.benchmark_group("Bench cmp");
//     // uses tmp
//     // let dir_path = tempfile::tempdir().unwrap().path();
//     // uses current directory, the generated files are kept
//     let dir_path = Path::new("");
//     // let curr = env::current_dir().unwrap();
//     // let dir_path = curr.as_path();
//     let num_lines = 100_000;
//     // The more differences, the faster cmp returns, as it stops after the first difference is found.
//     let num_differences = 1;
//
//     group.warm_up_time(Duration::from_millis(WARM_UP_TIME_MS));
//     // group.measurement_time(Duration::from_millis(MEASUREMENT_TIME_MS));
//     // group.sample_size(10);
//
//     let (from, to) =
//         generate_test_files(num_lines, 0, dir_path).expect("generate_test_files failed");
//     let cmd = format!("diff {from} {to}");
//     let opts = str_to_args(&cmd).into_iter().peekable();
//     let params = diffutilslib::params::parse_params(opts).unwrap();
//
//     // TODO need function because main cannot be called.
//     group.bench_function(format!("diff files unchanged, lines: {num_lines}"), |b| {
//         b.iter(|| diffutilslib::<diff>::cmp(&params).unwrap())
//     });
//
//     let (from, to) = generate_test_files(num_lines, num_differences, dir_path)
//         .expect("generate_test_files failed");
//     let cmd = format!("diff {from} {to} -s");
//     let opts = str_to_args(&cmd).into_iter().peekable();
//     let params = diffutilslib::params::parse_params(opts).unwrap();
//
//     // TODO need function because main cannot be called.
//     group.bench_function(format!("diff files changed, lines: {num_lines}"), |b| {
//         b.iter(|| diffutilslib::<diff>::cmp(&params).unwrap())
//     });
//
//     group.finish();
// }

fn cmp_parse_only(cmd: &str) -> String {
    let args = str_to_args(cmd).into_iter().peekable();
    let _params = match diffutilslib::cmp::parse_params(args) {
        Ok(params) => params,
        Err(e) => {
            return e.to_string();
        }
    };
    return "ok".to_string();
}

fn diff_parse_only(cmd: &str) -> String {
    let args = str_to_args(cmd).into_iter().peekable();
    let _params = match diffutilslib::params::parse_params(args) {
        Ok(params) => params,
        Err(e) => {
            return e.to_string();
        }
    };
    return "ok".to_string();
}

fn str_to_args(opt: &str) -> Vec<OsString> {
    let s: Vec<OsString> = opt
        .split(" ")
        .into_iter()
        .map(|s| OsString::from(s))
        .collect();

    s
}

/// Generates two test files for comparison.
///
/// Each line consists of 10 words with 5 letters, giving a line length of 60 bytes.
/// If num_differences is set, '*' will be inserted between the first two words of a line,
/// evenly spaced in the file. 1 will add the change in the last line, so the comparison takes longest.
fn generate_test_files(
    lines: usize,
    num_differences: usize,
    dir: &Path,
) -> std::io::Result<(String, String)> {
    let f1 = format!("{FROM_FILE}_{lines}.txt");
    let f2 = format!("{TO_FILE}_{lines}.txt");
    let from_path = dir.join(f1);
    let to_path = dir.join(f2);

    generate_file_fast(&from_path, &to_path, lines, num_differences)?;

    Ok((
        from_path.to_string_lossy().to_string(),
        to_path.to_string_lossy().to_string(),
    ))
}

// Largely Gemini AI Generated
fn generate_file_fast(
    from_name: &Path,
    to_name: &Path,
    line_count: usize,
    num_differences: usize,
) -> std::io::Result<()> {
    let file_from = File::create(from_name)?;
    let file_to = File::create(to_name)?;
    let change = if num_differences == 0 {
        0
    } else {
        line_count / num_differences
    };
    // Use a larger 128KB buffer for massive files
    let mut writer_from = BufWriter::with_capacity(128 * 1024, file_from);
    let mut writer_to = BufWriter::with_capacity(128 * 1024, file_to);
    let mut rng = rand::rng();

    // Each line: (5 chars * 10 words) + 9 spaces + 1 newline = 60 bytes
    let mut line_buffer = [b' '; 60];
    line_buffer[59] = b'\n'; // Set the newline once at the end

    for i in (0..line_count).rev() {
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
            if i % change == 0 {
                line_buffer[5] = b'*';
            }
            writer_to.write_all(&line_buffer)?;
            line_buffer[5] = b' ';
        }
    }

    writer_from.flush()?;
    writer_to.flush()?;

    Ok(())
}

#[allow(unused)]
// fn bench_binary_execution(c: &mut BenchmarkGroup<'_, WallTime>) {
fn bench_binary_execution_cmp(c: &mut Criterion) {
    c.bench_function("GNU cmp", |b| {
        b.iter(|| {
            let _status = Command::new("cmp")
                .arg("from_file_100000.txt")
                .arg("to_file_100000.txt")
                .arg("-s")
                .status()
                .expect("Failed to execute binary");

            // assert!(status.success());
        })
    });

    c.bench_function("cmp binary", |b| {
        b.iter(|| {
            let _status = Command::new("target/release/diffutils")
                .arg("cmp")
                .arg("from_file_100000.txt")
                .arg("to_file_100000.txt")
                .arg("-s")
                // .arg("--lines")
                // .arg(black_box("10000"))
                .status()
                .expect("Failed to execute binary");

            // assert!(status.success());
        })
    });
}
