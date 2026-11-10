// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use diffutilslib::assert_diff_eq;

use rstest::rstest;
use std::fs::File;
use uutests::{at_and_ts, new_ucmd};

#[rstest]
fn no_differences(#[values(&[][..], &["-u"], &["-c"], &["-e"])] format: &[&str]) {
    let (at, ts) = at_and_ts!();
    at.touch("file");
    ts.ucmd()
        .args(format)
        .args(&["file", "file"])
        .succeeds()
        .no_stdout();
}

// The same file, then two files with the same content.
#[rstest]
fn no_differences_report_identical_files(
    #[values(&[][..], &["-u"], &["-c"], &["-e"])] format: &[&str],
    #[values("file1", "file2")] second: &str,
) {
    let (at, ts) = at_and_ts!();
    at.write("file1", "foo\n");
    at.write("file2", "foo\n");
    ts.ucmd()
        .args(format)
        .args(&["-s", "file1", second])
        .succeeds()
        .stdout_is(format!("Files file1 and {second} are identical\n"));
}

#[rstest]
fn differences(#[values(&[][..], &["-u"], &["-c"], &["-e"])] format: &[&str]) {
    let (at, ts) = at_and_ts!();
    at.write("file1", "foo\n");
    at.write("file2", "bar\n");
    ts.ucmd()
        .args(format)
        .args(&["file1", "file2"])
        .fails_with_code(1)
        .stdout_check(|s| !s.is_empty());
}

#[rstest]
fn differences_brief(#[values(&[][..], &["-u"], &["-c"], &["-e"])] format: &[&str]) {
    let (at, ts) = at_and_ts!();
    at.write("file1", "foo\n");
    at.write("file2", "bar\n");
    ts.ucmd()
        .args(format)
        .args(&["-q", "file1", "file2"])
        .fails_with_code(1)
        .stdout_is("Files file1 and file2 differ\n");
}

#[test]
fn missing_newline() {
    let (at, ts) = at_and_ts!();
    at.write("file1", "foo");
    at.write("file2", "bar");
    ts.ucmd()
        .args(&["-e", "file1", "file2"])
        .fails_with_code(2)
        .stderr_str_check(|s| s.starts_with("No newline at end of file"));
}

#[rstest]
#[case::second(&["file1", "-"], "bar\n", "--- file1\tTIMESTAMP\n+++ -\tTIMESTAMP\n")]
#[case::first(&["-", "file2"], "foo\n", "--- -\tTIMESTAMP\n+++ file2\tTIMESTAMP\n")]
fn read_from_stdin(#[case] operands: &[&str], #[case] input: &str, #[case] headers: &str) {
    let (at, ts) = at_and_ts!();
    at.write("file1", "foo\n");
    at.write("file2", "bar\n");

    let result = ts
        .ucmd()
        .arg("-u")
        .args(operands)
        .pipe_in(input)
        .fails_with_code(1);
    assert_diff_eq!(
        result.stdout(),
        format!("{headers}@@ -1 +1 @@\n-foo\n+bar\n")
    );
}

#[cfg(unix)]
#[test]
fn read_from_dev_stdin() {
    let (at, ts) = at_and_ts!();
    at.write("file1", "foo\n");

    let result = ts
        .ucmd()
        .args(&["-u", "file1", "/dev/stdin"])
        .pipe_in("bar\n")
        .fails_with_code(1);
    assert_diff_eq!(
        result.stdout(),
        "--- file1\tTIMESTAMP\n+++ /dev/stdin\tTIMESTAMP\n@@ -1 +1 @@\n-foo\n+bar\n"
    );
}

#[test]
fn both_from_stdin() {
    new_ucmd!().args(&["-u", "-", "-"]).succeeds().no_stdout();
}

#[rstest]
fn missing_file_as_empty(#[values("-N", "--new-file", "--unidirectional-new-file")] option: &str) {
    let (at, ts) = at_and_ts!();
    at.write("present", "kiwi\nmango\n");

    ts.ucmd()
        .args(&[option, "absent", "present"])
        .fails_with_code(1)
        .stdout_is("0a1,2\n> kiwi\n> mango\n");
}

#[rstest]
#[case::new_file(&["-N", "present", "absent"], 1, "1,2d0\n< kiwi\n< mango\n")]
// Only a missing first operand is accepted here.
#[case::unidirectional(&["--unidirectional-new-file", "present", "absent"], 2, "")]
// Without the option, a missing file is trouble.
#[case::no_option(&["absent", "present"], 2, "")]
fn missing_second_file(#[case] args: &[&str], #[case] code: i32, #[case] expected: &str) {
    let (at, ts) = at_and_ts!();
    at.write("present", "kiwi\nmango\n");

    ts.ucmd().args(args).run().code_is(code).stdout_is(expected);
}

// TZ is not honored on Windows.
#[cfg(unix)]
#[rstest]
#[case::unified_c(
    "-u",
    "C",
    ["---", "+++"],
    "1970-01-01 00:00:00.000000000 +0000",
    "2009-02-13 23:31:30.250000000 +0000"
)]
#[case::context_utf8(
    "-c",
    "en_US.UTF-8",
    ["***", "---"],
    "1970-01-01 00:00:00.000000000 +0000",
    "2009-02-13 23:31:30.250000000 +0000"
)]
#[case::context_c(
    "-c",
    "C",
    ["***", "---"],
    "Thu Jan  1 00:00:00 1970",
    "Fri Feb 13 23:31:30 2009"
)]
fn missing_file_header_timestamp(
    #[case] format: &str,
    #[case] locale: &str,
    #[case] markers: [&str; 2],
    #[case] epoch: &str,
    #[case] timestamp: &str,
    // --unidirectional-new-file only accepts a missing first operand.
    #[values(
        ("-N", false),
        ("--new-file", false),
        ("--unidirectional-new-file", false),
        ("-N", true),
        ("--new-file", true)
    )]
    option_and_reverse: (&str, bool),
    #[values(false, true)] recursive: bool,
) {
    let (option, reverse) = option_and_reverse;
    let (at, ts) = at_and_ts!();
    at.mkdir("left");
    at.mkdir_all("right/nested");
    at.write("right/nested/file", "kiwi\n");
    let mtime = std::time::UNIX_EPOCH + std::time::Duration::from_millis(1_234_567_890_250);
    File::options()
        .write(true)
        .open(at.plus("right/nested/file"))
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    let absent = "left/nested/file";
    let present = "right/nested/file";

    let headers = if reverse {
        format!(
            "{} {present}\t{timestamp}\n{} {absent}\t{epoch}\n",
            markers[0], markers[1]
        )
    } else {
        format!(
            "{} {absent}\t{epoch}\n{} {present}\t{timestamp}\n",
            markers[0], markers[1]
        )
    };
    let (from, to) = if recursive {
        ("left", "right")
    } else {
        (absent, present)
    };
    let mut cmd = ts.ucmd();
    cmd.env("TZ", "UTC0")
        .env("LC_ALL", locale)
        .arg(format)
        .arg(option);
    if recursive {
        cmd.arg("-r");
    }
    cmd.arg(if reverse { to } else { from })
        .arg(if reverse { from } else { to })
        .fails_with_code(1)
        .no_stderr()
        .stdout_contains(&headers);
}

#[rstest]
fn missing_file_as_empty_keeps_stdin_timestamp(
    #[values("-u", "-c")] format: &str,
    #[values(["-", "file"], ["file", "-"])] operands: [&str; 2],
) {
    let (at, ts) = at_and_ts!();
    at.write("file", "file\n");

    ts.ucmd()
        .args(&[format, "-N"])
        .args(&operands)
        .pipe_in("stdin\n")
        .fails_with_code(1)
        .no_stderr()
        .stdout_does_not_contain("1970");
}

#[test]
fn binary_files() {
    let (at, ts) = at_and_ts!();
    at.write_bytes("left.bin", b"header\x00\x7fpayload-one\n");
    at.write_bytes("right.bin", b"header\x00\x7fpayload-two\n");

    ts.ucmd()
        .args(&["left.bin", "right.bin"])
        .fails_with_code(1)
        .stdout_only("Binary files left.bin and right.bin differ\n");

    // Identical binary inputs are not reported.
    ts.ucmd()
        .args(&["left.bin", "-"])
        .pipe_in(b"header\x00\x7fpayload-one\n".to_vec())
        .succeeds()
        .no_stdout();

    // -q keeps its usual wording.
    ts.ucmd()
        .args(&["-q", "left.bin", "right.bin"])
        .fails_with_code(1)
        .stdout_only("Files left.bin and right.bin differ\n");
}

// -a and --text force a line-by-line comparison.
#[rstest]
fn binary_files_as_text(#[values("-a", "--text")] option: &str) {
    let (at, ts) = at_and_ts!();
    at.write_bytes("left.bin", b"header\x00\x7fpayload-one\n");
    at.write_bytes("right.bin", b"header\x00\x7fpayload-two\n");

    ts.ucmd()
        .args(&[option, "left.bin", "right.bin"])
        .fails_with_code(1)
        .stdout_str_check(|s| s.starts_with("1c1\n"));
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn stdin_is_directory() {
    let (at, ts) = at_and_ts!();
    at.mkdir("dir");
    at.write("dir/notes", "alpha\n");
    at.write("notes", "beta\n");

    // The directory itself must be standard input, not its contents.
    ts.ucmd()
        .args(&["-", "notes"])
        .set_stdin(File::open(at.plus("dir")).unwrap())
        .fails_with_code(1)
        .stdout_is("1c1\n< alpha\n---\n> beta\n");
}

const PLAIN: &str = "2c2\n< rain\n---\n> snow\n";
const COLORED: &str = "\x1b[36m2c2\x1b[0m\n\x1b[31m< rain\x1b[0m\n---\n\x1b[32m> snow\x1b[0m\n";

#[rstest]
#[case::always(&["--color=always"], None, COLORED)]
#[case::palette(
    &["--color=always", "--palette=de=4:ad=7"],
    None,
    "\x1b[36m2c2\x1b[0m\n\x1b[4m< rain\x1b[0m\n---\n\x1b[7m> snow\x1b[0m\n"
)]
#[case::palette_before_color(
    &["--palette", "ln=1", "--color=always"],
    None,
    "\x1b[1m2c2\x1b[0m\n\x1b[31m< rain\x1b[0m\n---\n\x1b[32m> snow\x1b[0m\n"
)]
// Standard output is not a terminal here, and TERM=dumb never gets colors.
#[case::auto_not_a_terminal(&["--color"], Some("dumb"), PLAIN)]
#[case::auto_dumb_terminal(&["--color=auto", "---presume-output-tty"], Some("dumb"), PLAIN)]
// Like GNU diff, an unset TERM does not disable --color=auto on a terminal.
#[case::auto_unset_term(&["--color=auto", "---presume-output-tty"], None, COLORED)]
fn color(#[case] options: &[&str], #[case] term: Option<&str>, #[case] expected: &str) {
    let (at, ts) = at_and_ts!();
    at.write("old", "sun\nrain\n");
    at.write("new", "sun\nsnow\n");

    // The environment of the command is cleared, so TERM is only set here.
    let mut cmd = ts.ucmd();
    if let Some(term) = term {
        cmd.env("TERM", term);
    }
    cmd.args(options)
        .args(&["old", "new"])
        .fails_with_code(1)
        .stdout_only(expected);
}

// An invalid palette is reported and disables colors.
#[test]
fn color_invalid_palette() {
    let (at, ts) = at_and_ts!();
    at.write("old", "sun\nrain\n");
    at.write("new", "sun\nsnow\n");

    ts.ucmd()
        .args(&["--color=always", "--palette=up=1", "old", "new"])
        .fails_with_code(1)
        .stdout_is(PLAIN)
        .stderr_contains("unrecognized prefix: up");
}

#[rstest]
#[case::invalid_value("--color=sometimes", "invalid color 'sometimes'")]
#[case::misspelled("--colour", "unrecognized option '--colour'")]
fn color_bad_option(#[case] option: &str, #[case] error: &str) {
    let (at, ts) = at_and_ts!();
    at.write("old", "sun\nrain\n");
    at.write("new", "sun\nsnow\n");

    ts.ucmd()
        .args(&[option, "old", "new"])
        .fails_with_code(2)
        .stderr_contains(error);
}

#[cfg(target_os = "linux")]
#[test]
fn write_error() {
    let (at, ts) = at_and_ts!();
    at.write("old", "left\n");
    at.write("new", "right\n");

    ts.ucmd()
        .args(&["old", "new"])
        .set_stdout(File::create("/dev/full").unwrap())
        .fails_with_code(2)
        .stderr_is("diff: standard output: No space left on device\n");
}

const TRADITIONAL: &str = "Fri Feb 13 23:31:30 2009";
const ISO: &str = "2009-02-13 23:31:30.250000000 +0000";

// LC_ALL overrides LC_TIME, which overrides LANG; empty values are unset.
// TZ is not honored on Windows.
#[cfg(unix)]
#[rstest]
#[case::lc_all_c(&[("LC_ALL", "C")], TRADITIONAL)]
#[case::lc_all_posix(&[("LC_ALL", "POSIX"), ("LC_TIME", "en_US.UTF-8")], TRADITIONAL)]
#[case::lc_all_utf8(&[("LC_ALL", "en_US.UTF-8"), ("LC_TIME", "C")], ISO)]
#[case::lc_time_c(&[("LC_TIME", "C"), ("LANG", "en_US.UTF-8")], TRADITIONAL)]
#[case::lc_time_utf8(&[("LC_TIME", "en_US.UTF-8"), ("LANG", "C")], ISO)]
#[case::lang_posix(&[("LC_ALL", ""), ("LC_TIME", ""), ("LANG", "POSIX")], TRADITIONAL)]
#[case::lang_utf8(&[("LANG", "fr_FR.UTF-8")], ISO)]
fn context_timestamp_follows_time_locale(
    #[case] variables: &[(&str, &str)],
    #[case] timestamp: &str,
) {
    let (at, ts) = at_and_ts!();
    at.write("old", "left\n");
    at.write("new", "right\n");
    // 2009-02-13 23:31:30.25 UTC
    let mtime = std::time::UNIX_EPOCH + std::time::Duration::from_millis(1_234_567_890_250);
    for path in ["old", "new"] {
        File::options()
            .write(true)
            .open(at.plus(path))
            .unwrap()
            .set_modified(mtime)
            .unwrap();
    }

    let expected = format!("*** old\t{timestamp}\n--- new\t{timestamp}\n");
    // The command starts with a cleared environment except for the
    // LC_ALL=C default, which an empty value unsets.
    ts.ucmd()
        .env("TZ", "UTC0")
        .env("LC_ALL", "")
        .envs(variables.iter().copied())
        .args(&["-c", "old", "new"])
        .fails_with_code(1)
        .stdout_str_check(|s| s.starts_with(&expected));
}

#[test]
fn compare_directories() {
    let (at, ts) = at_and_ts!();
    let left = "left";
    let right = "right";
    for dir in [left, right] {
        at.mkdir_all(&format!("{dir}/nested/inner"));
        at.write(&format!("{dir}/equal"), "same\n");
    }
    at.write("left/changed", "old\n");
    at.write("right/changed", "new\n");
    at.write("left/nested/inner/deep", "one\n");
    at.write("right/nested/inner/deep", "two\n");
    at.write("left/gone", "bye\n");
    at.write("right/added", "hi\n");
    at.write("left/kind", "file\n");
    at.mkdir("right/kind");
    // Entry names are joined to the directories with the native separator.
    let s = std::path::MAIN_SEPARATOR;

    ts.ucmd()
        .args(&[left, right])
        .fails_with_code(1)
        .stdout_is(format!(
            "Only in {right}: added\n\
             diff {left}{s}changed {right}{s}changed\n1c1\n< old\n---\n> new\n\
             Only in {left}: gone\n\
             File {left}{s}kind is a regular file while file {right}{s}kind is a directory\n\
             Common subdirectories: {left}{s}nested and {right}{s}nested\n"
        ));

    // Options are repeated as given in the headers, and trailing slashes
    // do not end up in the compared paths.
    ts.ucmd()
        .args(&["-r", "-q", &format!("{left}/"), right])
        .fails_with_code(1)
        .stdout_is(format!(
            "Only in {right}: added\n\
             Files {left}{s}changed and {right}{s}changed differ\n\
             Only in {left}/: gone\n\
             File {left}{s}kind is a regular file while file {right}{s}kind is a directory\n\
             Files {left}{s}nested{s}inner{s}deep and {right}{s}nested{s}inner{s}deep differ\n"
        ));

    ts.ucmd()
        .args(&["-N", "-r", "-e", left, right])
        .fails_with_code(1)
        .stdout_is(format!(
            "diff -N -r -e {left}{s}added {right}{s}added\n0a\nhi\n.\n\
             diff -N -r -e {left}{s}changed {right}{s}changed\n1c\nnew\n.\n\
             diff -N -r -e {left}{s}gone {right}{s}gone\n1d\n\
             File {left}{s}kind is a regular file while file {right}{s}kind is a directory\n\
             diff -N -r -e {left}{s}nested{s}inner{s}deep {right}{s}nested{s}inner{s}deep\n1c\ntwo\n.\n"
        ));

    // -s reports the identical files, but a directory is not compared with
    // itself.
    ts.ucmd()
        .args(&["-r", "-s", left, left])
        .succeeds()
        .no_stdout();

    let copy = "copy";
    at.mkdir_all("copy/nested/inner");
    at.write("copy/changed", "old\n");
    at.write("copy/equal", "same\n");
    at.write("copy/nested/inner/deep", "one\n");
    ts.ucmd()
        .args(&["-r", "-s", left, copy])
        .fails_with_code(1)
        .stdout_is(format!(
            "Files {left}{s}changed and {copy}{s}changed are identical\n\
             Files {left}{s}equal and {copy}{s}equal are identical\n\
             Only in {left}: gone\n\
             Only in {left}: kind\n\
             Files {left}{s}nested{s}inner{s}deep and {copy}{s}nested{s}inner{s}deep are identical\n"
        ));
}

#[cfg(unix)]
#[test]
fn compare_directories_special_entries() {
    let (at, ts) = at_and_ts!();
    for dir in ["one", "two"] {
        at.mkdir_all(&format!("{dir}/tools/bin"));
        // Links to the same directory are not compared.
        at.relative_symlink_dir("..", &format!("{dir}/parent"));
        at.mkfifo(&format!("{dir}/queue"));
    }
    at.write("one/tools/bin/run", "go\n");
    std::fs::remove_dir_all(at.plus("two/tools")).unwrap();

    ts.ucmd()
        .args(&["-N", "one", "two"])
        .fails_with_code(1)
        .stdout_is(
            "File one/queue is a fifo while file two/queue is a fifo\n\
             Common subdirectories: one/tools and two/tools\n",
        );

    ts.ucmd()
        .args(&["-r", "-N", "one", "two"])
        .fails_with_code(1)
        .stdout_is(
            "File one/queue is a fifo while file two/queue is a fifo\n\
             diff -r -N one/tools/bin/run two/tools/bin/run\n1d0\n< go\n",
        );

    // A link back to a directory being compared is reported.
    at.relative_symlink_dir("..", "one/tools/again");
    at.mkdir("two/tools");
    at.relative_symlink_dir("..", "two/tools/again");
    ts.ucmd()
        .args(&["-r", "one", "two"])
        .fails_with_code(2)
        .stdout_contains("Only in one/tools: bin\n")
        .stderr_is("diff: one/tools/again: recursive directory loop\n");
}

#[test]
fn directory_name_matching() {
    let (at, ts) = at_and_ts!();
    let left = "left";
    let right = "right";
    at.mkdir_all("left/lib");
    at.mkdir_all("right/lib");
    for name in ["Readme", "notes", "lib/early"] {
        at.write(&format!("{left}/{name}"), "v1\n");
    }
    for name in ["readme", "Notes", "Todo", "lib/EARLY"] {
        at.write(&format!("{right}/{name}"), "v1\n");
    }
    // Names differing only by case can only coexist on case-sensitive file
    // systems; there, they pair in byte order.
    at.write("left/README", "v1\n");
    let case_sensitive = std::fs::read_dir(at.plus(left)).unwrap().count() == 4;
    let unpaired_readme = if case_sensitive {
        format!("Only in {left}: Readme\n")
    } else {
        String::new()
    };
    let s = std::path::MAIN_SEPARATOR;

    ts.ucmd()
        .args(&["-r", "--ignore-file-name-case", left, right])
        .fails_with_code(1)
        .stdout_is(format!("{unpaired_readme}Only in {right}: Todo\n"));

    // -S only skips names at the top level.
    ts.ucmd()
        .args(&["-r", "-S", "l", left, right])
        .fails_with_code(1)
        .stdout_is(format!(
            "Only in {right}{s}lib: EARLY\nOnly in {left}{s}lib: early\n\
             Only in {left}: notes\nOnly in {right}: readme\n"
        ));

    // The starting name is matched regardless of case too.
    ts.ucmd()
        .args(&[
            "--starting-file=NOTES",
            "--ignore-file-name-case",
            left,
            right,
        ])
        .fails_with_code(1)
        .stdout_is(format!("{unpaired_readme}Only in {right}: Todo\n"));

    // A file compared with a directory finds its counterpart regardless of case.
    ts.ucmd()
        .args(&["--ignore-file-name-case", &format!("{left}{s}notes"), right])
        .succeeds()
        .no_stdout();
}

#[cfg(unix)]
#[test]
fn no_dereference() {
    let (at, ts) = at_and_ts!();
    at.write("plain", "text\n");
    at.write("other", "text\n");
    at.relative_symlink_file("plain", "to-plain");
    at.relative_symlink_file("plain", "also-to-plain");
    at.relative_symlink_file("other", "to-other");

    ts.ucmd()
        .args(&["plain", "to-other"])
        .succeeds()
        .no_stdout();

    ts.ucmd()
        .args(&["--no-dereference", "plain", "to-other"])
        .fails_with_code(1)
        .stdout_is("File plain is a regular file while file to-other is a symbolic link\n");

    ts.ucmd()
        .args(&["--no-dereference", "to-plain", "also-to-plain"])
        .succeeds()
        .no_stdout();

    ts.ucmd()
        .args(&["--no-dereference", "to-plain", "to-other"])
        .fails_with_code(1)
        .stdout_is("Symbolic links 'to-plain' -> 'plain' and 'to-other' -> 'other' differ\n");

    // In directories, dangling links are entries like any other.
    at.mkdir("left");
    at.mkdir("right");
    at.relative_symlink_file("missing", "left/dangling");
    at.relative_symlink_file("../plain", "right/dangling");
    ts.ucmd()
        .args(&["-r", "--no-dereference", "left", "right"])
        .fails_with_code(1)
        .stdout_is(
            "Symbolic links 'left/dangling' -> 'missing' and 'right/dangling' -> '../plain' differ\n",
        );

    ts.ucmd()
        .args(&["-r", "left", "right"])
        .fails_with_code(2)
        .stderr_contains("dangling: No such file or directory");

    // Links with the same target are identical files.
    ts.ucmd()
        .args(&["-s", "--no-dereference", "to-plain", "also-to-plain"])
        .succeeds()
        .stdout_is("Files to-plain and also-to-plain are identical\n");

    // A file compared with a directory is looked up in it, and that entry is
    // not followed either.
    at.write("left/plain", "text\n");
    at.relative_symlink_file("../other", "left/to-other");
    ts.ucmd()
        .args(&["--no-dereference", "plain", "left"])
        .succeeds()
        .no_stdout();

    ts.ucmd()
        .args(&["--no-dereference", "left", "to-other"])
        .fails_with_code(1)
        .stdout_is(
            "Symbolic links 'left/to-other' -> '../other' and 'to-other' -> 'other' differ\n",
        );
}

#[test]
fn compare_file_to_directory() {
    let (at, ts) = at_and_ts!();
    at.mkdir("d");
    at.write("a", "a\n");
    at.write("d/a", "da\n");
    // The name is joined to the directory with the native separator.
    let da = format!("d{}a", std::path::MAIN_SEPARATOR);

    let result = ts.ucmd().args(&["-u", "d", "a"]).fails_with_code(1);
    assert_diff_eq!(
        result.stdout(),
        format!("--- {da}\tTIMESTAMP\n+++ a\tTIMESTAMP\n@@ -1 +1 @@\n-da\n+a\n")
    );

    let result = ts.ucmd().args(&["-u", "a", "d"]).fails_with_code(1);
    assert_diff_eq!(
        result.stdout(),
        format!("--- a\tTIMESTAMP\n+++ {da}\tTIMESTAMP\n@@ -1 +1 @@\n-a\n+da\n")
    );
}

// Regression for #269: the `-y` gutter marker must sit at the middle of the gutter.
#[rstest]
// --expand-tabs: padding is spaces.
#[case::expand_tabs(&["--expand-tabs"], "aa                 <\n                   >    bb\n")]
// Default (expand_tabs: false): padding goes through the tab path.
#[case::tabs(&[], "aa\t\t   <\n\t\t   >\tbb\n")]
fn sdiff_gutter_marker_column(#[case] options: &[&str], #[case] expected: &str) {
    let (at, ts) = at_and_ts!();
    at.write("file1", "aa\n");
    at.write("file2", "bb\n");

    ts.ucmd()
        .args(&["-y", "--width=40"])
        .args(options)
        .args(&["file1", "file2"])
        .run()
        .stdout_is(expected);
}

#[rstest]
#[case::delete_and_append(
    "north\neast\nsouth\nwest\n",
    "north\nsouth\nwest\nup\ndown\n",
    "2d1\n< east\n4a4,5\n> up\n> down\n"
)]
#[case::append_to_empty("", "north\n", "0a1\n> north\n")]
#[case::append_one(
    "north\neast\nsouth\nwest\n",
    "north\neast\nsouth\nwest\nup\n",
    "4a5\n> up\n"
)]
fn normal_format_single_line_ranges(#[case] old: &str, #[case] new: &str, #[case] expected: &str) {
    let (at, ts) = at_and_ts!();
    at.write("old", old);
    at.write("new", new);

    ts.ucmd()
        .args(&["old", "new"])
        .fails_with_code(1)
        .stdout_is(expected);
}

#[rstest]
#[case::single_line_deletion(
    "red\norange\nyellow\ngreen\nblue\n",
    "red\norange\nyellow\nblue\n",
    "4d\n"
)]
#[case::range_deletion("red\norange\nyellow\ngreen\nblue\n", "red\nblue\n", "2,4d\n")]
// GNU diff starts from the end of the file, so every command uses the line
// numbers of the original file.
#[case::deletions_last_to_first("a\nb\nc\nd\ne\n", "b\nd\n", "5d\n3d\n1d\n")]
#[case::appends_last_to_first("a\nb\n", "X\na\nb\nY\n", "2a\nY\n.\n0a\nX\n.\n")]
#[case::change_after_deletion("a\nb\nc\nd\n", "a\nX\nY\nc\n", "4d\n2c\nX\nY\n.\n")]
// A "." line is written as "..", then s/.// fixes it. Like GNU diff, no empty
// "a" command follows when it is the last line of the hunk.
#[case::dot_line_appended("a\nb\n", "a\nb\n.\n", "2a\n..\n.\ns/.//\n")]
#[case::dot_lines_changed("a\nb\n", ".\n.\nb\n", "1c\n..\n.\ns/.//\na\n..\n.\ns/.//\n")]
#[case::dot_line_in_change("a\nb\n", "a\n.\nc\n", "2c\n..\n.\ns/.//\na\nc\n.\n")]
fn ed_format(#[case] old: &str, #[case] new: &str, #[case] expected: &str) {
    let (at, ts) = at_and_ts!();
    at.write("old", old);
    at.write("new", new);

    ts.ucmd()
        .args(&["-e", "old", "new"])
        .fails_with_code(1)
        .stdout_is(expected);
}

#[test]
fn function_lines() {
    let (at, ts) = at_and_ts!();
    at.write(
        "old.c",
        "int zero;\nstatic int twice(int n)\n{\n  int r;\n  r = n * 2;\n  return r;\n}\n",
    );
    at.write(
        "new.c",
        "int zero;\nstatic int twice(int n)\n{\n  int r;\n  r = n + n;\n  return r;\n}\n",
    );

    // -p defaults to the context format and shows the enclosing C function.
    ts.ucmd()
        .args(&["-p", "-C1", "old.c", "new.c"])
        .fails_with_code(1)
        .stdout_contains("\n*************** static int twice(int n)\n*** 4,6 ****\n");

    ts.ucmd()
        .args(&["-U0", "-F", "^int", "old.c", "new.c"])
        .fails_with_code(1)
        .stdout_str_check(|s| s.ends_with("@@ -5 +5 @@ int zero;\n-  r = n * 2;\n+  r = n + n;\n"));

    ts.ucmd()
        .args(&["-u", "-F", "\\(", "old.c", "new.c"])
        .fails_with_code(2)
        .stderr_is("'\\(': Unmatched ( or \\(\n");
}

#[cfg(unix)]
#[test]
fn quoted_file_names() {
    let (at, ts) = at_and_ts!();
    at.write("my notes", "draft\n");
    at.write("final\tv2", "done\n");
    at.write("plain$name", "done\n");

    ts.ucmd()
        .args(&["-u", "my notes", "final\tv2"])
        .fails_with_code(1)
        .stdout_matches(
            &regex::Regex::new("^--- \"my notes\"\t[^\n]*\n\\+\\+\\+ \"final\\\\tv2\"\t").unwrap(),
        );

    // Names without special characters are left alone.
    ts.ucmd()
        .args(&["-c", "my notes", "plain$name"])
        .fails_with_code(1)
        .stdout_contains("\n--- plain$name\t");

    // The diff -r header quotes the options for the shell and the names as
    // in the other headers.
    for (side, content) in [("one", "a\n"), ("two", "b\n")] {
        at.mkdir(side);
        at.write(&format!("{side}/my file"), content);
    }
    ts.ucmd()
        .args(&["-r", "-F", "my func", "one", "two"])
        .fails_with_code(1)
        .stdout_is("diff -r -F 'my func' \"one/my file\" \"two/my file\"\n1c1\n< a\n---\n> b\n");
}

#[rstest]
#[case::after_double_dash(&["--", "old", "new", "extra"])]
#[case::without_double_dash(&["old", "new", "extra"])]
fn extra_operand(#[case] args: &[&str]) {
    let (at, ts) = at_and_ts!();
    at.write("old", "a\nb\n");
    at.write("new", "a\nc\n");
    ts.ucmd()
        .args(args)
        .fails_with_code(2)
        .no_stdout()
        .stderr_is("diff: extra operand 'extra'\ndiff: Try 'diff --help' for more information.\n");
}

#[rstest]
#[case::unified("-u", "@@ -1,5 +1,5 @@\n 1\n-2\n+X\n 3\n 4\n 5\n")]
#[case::context(
    "-c",
    "*** 1,5 ****\n  1\n! 2\n  3\n  4\n  5\n--- 1,5 ----\n  1\n! X\n  3\n  4\n  5\n"
)]
// the last line is still shown when it is within the trailing context
#[case::up_to_last_line("-U5", "@@ -1,7 +1,7 @@\n 1\n-2\n+X\n 3\n 4\n 5\n 6\n 7\n")]
#[case::last_line(
    "-U6",
    "@@ -1,8 +1,8 @@\n 1\n-2\n+X\n 3\n 4\n 5\n 6\n 7\n 8\n\\ No newline at end of file\n"
)]
fn trailing_context_without_final_newlines(#[case] format: &str, #[case] hunk: &str) {
    let (at, ts) = at_and_ts!();
    at.write("file1", "1\n2\n3\n4\n5\n6\n7\n8");
    at.write("file2", "1\nX\n3\n4\n5\n6\n7\n8");

    ts.ucmd()
        .args(&[format, "file1", "file2"])
        .fails_with_code(1)
        .stdout_str_check(|s| s.ends_with(hunk));
}
