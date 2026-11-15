// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use uutests::{at_and_ts, new_ucmd};

#[test]
fn test_sdiff_missing_operands() {
    new_ucmd!()
        .fails_with_code(2)
        .stderr_contains("Usage: sdiff <from> <to>");

    new_ucmd!()
        .arg("only_one_file")
        .fails_with_code(2)
        .stderr_contains("Usage: sdiff <from> <to>");
}

#[test]
fn test_sdiff_extra_operands() {
    let (at, ts) = at_and_ts!();
    at.touch("a");
    at.touch("b");
    at.touch("c");

    ts.ucmd()
        .args(&["a", "b", "c"])
        .fails_with_code(2)
        .stderr_contains("Usage: sdiff <from> <to>");
}

#[test]
fn test_sdiff_unrecognized_option() {
    let (at, ts) = at_and_ts!();
    at.touch("a");
    at.touch("b");

    ts.ucmd()
        .args(&["--unknown-flag", "a", "b"])
        .fails_with_code(2)
        .stderr_contains("unrecognized option '--unknown-flag'");

    ts.ucmd()
        .args(&["-z", "a", "b"])
        .fails_with_code(2)
        .stderr_contains("unrecognized option '-z'");
}

#[test]
fn test_sdiff_double_dash_operand_separator() {
    let (at, ts) = at_and_ts!();
    at.write("-odd-name1", "test content\n");
    at.write("-odd-name2", "test content\n");

    ts.ucmd()
        .args(&["--", "-odd-name1", "-odd-name2"])
        .succeeds()
        .stdout_contains("test content");
}

#[test]
fn test_sdiff_nonexistent_file() {
    new_ucmd!()
        .args(&["no_such_file1", "no_such_file2"])
        .fails_with_code(2);
}

#[test]
fn test_sdiff_identical_files() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "alpha\nbeta\ngamma\n");
    at.write("f2.txt", "alpha\nbeta\ngamma\n");

    let result = ts.ucmd().args(&["f1.txt", "f2.txt"]).succeeds();
    let out = result.stdout_str();
    assert!(out.contains("alpha"));
    assert!(out.contains("beta"));
    assert!(out.contains("gamma"));
    // Identical lines have whitespace gutter (no < or > markers)
    assert!(!out.contains("<"));
    assert!(!out.contains(">"));
}

#[test]
fn test_sdiff_different_files() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "apple\nbanana\n");
    at.write("f2.txt", "apple\ncherry\n");

    let result = ts.ucmd().args(&["f1.txt", "f2.txt"]).fails_with_code(1);
    let out = result.stdout_str();
    assert!(out.contains("apple"));
    assert!(out.contains("<"));
    assert!(out.contains(">"));
    assert!(out.contains("banana"));
    assert!(out.contains("cherry"));
}

#[test]
fn test_sdiff_empty_files() {
    let (at, ts) = at_and_ts!();
    at.touch("empty1.txt");
    at.touch("empty2.txt");

    // Both empty: identical -> exit 0
    let res = ts.ucmd().args(&["empty1.txt", "empty2.txt"]).succeeds();
    assert_eq!(res.stdout_str(), "");

    // One empty, one non-empty: different -> exit 1
    at.write("non_empty.txt", "hello\n");
    let res_diff = ts
        .ucmd()
        .args(&["empty1.txt", "non_empty.txt"])
        .fails_with_code(1);
    assert!(res_diff.stdout_str().contains(">"));
    assert!(res_diff.stdout_str().contains("hello"));
}

#[test]
fn test_sdiff_suppress_common_lines_flag() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "same_line\nleft_only\n");
    at.write("f2.txt", "same_line\nright_only\n");

    let result = ts
        .ucmd()
        .args(&["-s", "f1.txt", "f2.txt"])
        .fails_with_code(1);
    let out = result.stdout_str();
    assert!(
        !out.contains("same_line"),
        "Common lines must be omitted with -s"
    );
    assert!(out.contains("left_only"));
    assert!(out.contains("right_only"));

    let result_long = ts
        .ucmd()
        .args(&["--suppress-common-lines", "f1.txt", "f2.txt"])
        .fails_with_code(1);
    assert!(!result_long.stdout_str().contains("same_line"));
}

#[test]
fn test_sdiff_suppress_common_lines_identical_files() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "identical\n");
    at.write("f2.txt", "identical\n");

    let result = ts.ucmd().args(&["-s", "f1.txt", "f2.txt"]).succeeds();
    assert_eq!(
        result.stdout_str(),
        "",
        "Output should be empty for identical files with -s"
    );
}

#[test]
fn test_sdiff_left_column_flag() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "shared_line\n");
    at.write("f2.txt", "shared_line\n");

    // GNU sdiff -l formats matching lines with '(' in the midpoint gutter
    let result = ts
        .ucmd()
        .args(&["-l", "--width=80", "--expand-tabs", "f1.txt", "f2.txt"])
        .succeeds();
    assert_eq!(
        result.stdout_str(),
        "shared_line                           (\n"
    );

    let result_long = ts
        .ucmd()
        .args(&[
            "--left-column",
            "--width=80",
            "--expand-tabs",
            "f1.txt",
            "f2.txt",
        ])
        .succeeds();
    assert_eq!(
        result_long.stdout_str(),
        "shared_line                           (\n"
    );
}

#[test]
fn test_sdiff_left_column_with_differences() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "common\nleft_only\n");
    at.write("f2.txt", "common\nright_only\n");

    let res = ts
        .ucmd()
        .args(&["-l", "--width=40", "--expand-tabs", "f1.txt", "f2.txt"])
        .fails_with_code(1);
    let out = res.stdout_str();
    assert!(out.contains("("));
    assert!(out.contains("<"));
    assert!(out.contains(">"));
}

#[test]
fn test_sdiff_custom_width_flags() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "a\n");
    at.write("f2.txt", "b\n");

    // Detached short: -w 80
    ts.ucmd()
        .args(&["-w", "80", "f1.txt", "f2.txt"])
        .fails_with_code(1);

    // Attached short: -w80
    ts.ucmd()
        .args(&["-w80", "f1.txt", "f2.txt"])
        .fails_with_code(1);

    // Long flag with =: --width=80
    ts.ucmd()
        .args(&["--width=80", "f1.txt", "f2.txt"])
        .fails_with_code(1);

    // Long flag with space: --width 80
    ts.ucmd()
        .args(&["--width", "80", "f1.txt", "f2.txt"])
        .fails_with_code(1);
}

#[test]
fn test_sdiff_invalid_width() {
    let (at, ts) = at_and_ts!();
    at.touch("f1.txt");
    at.touch("f2.txt");

    // Width less than GNU minimum (7)
    ts.ucmd()
        .args(&["-w", "5", "f1.txt", "f2.txt"])
        .fails_with_code(2)
        .stderr_contains("invalid width '5'");

    ts.ucmd()
        .args(&["-w0", "f1.txt", "f2.txt"])
        .fails_with_code(2)
        .stderr_contains("invalid width '0'");

    // Non-numeric width
    ts.ucmd()
        .args(&["--width=abc", "f1.txt", "f2.txt"])
        .fails_with_code(2)
        .stderr_contains("invalid width 'abc'");

    // Missing width argument
    ts.ucmd()
        .args(&["-w"])
        .fails_with_code(2)
        .stderr_contains("option '-w' requires an argument");

    ts.ucmd()
        .args(&["--width"])
        .fails_with_code(2)
        .stderr_contains("option '--width' requires an argument");
}

#[test]
fn test_sdiff_expand_tabs_flag() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "a\tb\n");
    at.write("f2.txt", "a\tb\n");

    let result = ts
        .ucmd()
        .args(&["-t", "--width=80", "-l", "f1.txt", "f2.txt"])
        .succeeds();
    assert_eq!(
        result.stdout_str(),
        "a       b                             (\n"
    );

    let result_long = ts
        .ucmd()
        .args(&["--expand-tabs", "--width=80", "-l", "f1.txt", "f2.txt"])
        .succeeds();
    assert_eq!(
        result_long.stdout_str(),
        "a       b                             (\n"
    );
}

#[test]
fn test_sdiff_tabsize() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "a\tb\n");
    at.write("f2.txt", "a\tb\n");

    // --tabsize=4 with tab expansion
    let res = ts
        .ucmd()
        .args(&["-t", "--tabsize=4", "--width=40", "-l", "f1.txt", "f2.txt"])
        .succeeds();
    assert_eq!(res.stdout_str(), "a   b             (\n");

    // --tabsize 4 (detached)
    let res_detached = ts
        .ucmd()
        .args(&[
            "-t",
            "--tabsize",
            "4",
            "--width=40",
            "-l",
            "f1.txt",
            "f2.txt",
        ])
        .succeeds();
    assert_eq!(res_detached.stdout_str(), "a   b             (\n");

    // Invalid tabsize 0
    ts.ucmd()
        .args(&["--tabsize=0", "f1.txt", "f2.txt"])
        .fails_with_code(2)
        .stderr_contains("invalid tabsize '0'");

    ts.ucmd()
        .args(&["--tabsize", "0", "f1.txt", "f2.txt"])
        .fails_with_code(2)
        .stderr_contains("invalid tabsize '0'");

    // Missing argument
    ts.ucmd()
        .args(&["--tabsize"])
        .fails_with_code(2)
        .stderr_contains("option '--tabsize' requires an argument");
}

#[test]
fn test_sdiff_binary_files() {
    let (at, ts) = at_and_ts!();
    at.write_bytes("b1.bin", b"\0first binary\n");
    at.write_bytes("b2.bin", b"\0second binary\n");

    // GNU diffutils outputs "Binary files ... differ" to stdout
    ts.ucmd()
        .args(&["b1.bin", "b2.bin"])
        .fails_with_code(1)
        .stdout_contains("Binary files b1.bin and b2.bin differ");
}

#[test]
fn test_sdiff_binary_files_identical() {
    let (at, ts) = at_and_ts!();
    at.write_bytes("b1.bin", b"\0identical binary data\n");
    at.write_bytes("b2.bin", b"\0identical binary data\n");

    ts.ucmd().args(&["b1.bin", "b2.bin"]).succeeds();
}

#[test]
fn test_sdiff_text_flag_overrides_binary() {
    let (at, ts) = at_and_ts!();
    at.write_bytes("b1.bin", b"\0first binary\n");
    at.write_bytes("b2.bin", b"\0second binary\n");

    // Short flag -a
    ts.ucmd()
        .args(&["-a", "b1.bin", "b2.bin"])
        .fails_with_code(1)
        .stdout_contains("<");

    // Long flag --text
    ts.ucmd()
        .args(&["--text", "b1.bin", "b2.bin"])
        .fails_with_code(1)
        .stdout_contains("<");
}

#[test]
fn test_sdiff_stdin_left_operand() {
    let (at, ts) = at_and_ts!();
    at.write("f2.txt", "line from disk\n");

    let res = ts
        .ucmd()
        .args(&["-", "f2.txt"])
        .pipe_in("line from stdin\n")
        .fails_with_code(1);
    let out = res.stdout_str();
    assert!(out.contains("line from stdin"));
    assert!(out.contains("line from disk"));
    assert!(out.contains("<"));
    assert!(out.contains(">"));
}

#[test]
fn test_sdiff_stdin_right_operand() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "line from disk\n");

    let res = ts
        .ucmd()
        .args(&["f1.txt", "-"])
        .pipe_in("line from stdin\n")
        .fails_with_code(1);
    let out = res.stdout_str();
    assert!(out.contains("line from disk"));
    assert!(out.contains("line from stdin"));
}

#[test]
fn test_sdiff_stdin_identical() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "same content\n");

    let res = ts
        .ucmd()
        .args(&["-", "f1.txt"])
        .pipe_in("same content\n")
        .succeeds();
    assert!(res.stdout_str().contains("same content"));
    assert!(!res.stdout_str().contains("<"));
    assert!(!res.stdout_str().contains(">"));
}

#[test]
fn test_sdiff_both_from_stdin() {
    // Both operands as "-" denotes comparing stdin with itself (identical)
    new_ucmd!().args(&["-", "-"]).succeeds();
}

#[test]
fn test_sdiff_missing_trailing_newline() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "no newline");
    at.write("f2.txt", "no newline");

    ts.ucmd().args(&["f1.txt", "f2.txt"]).succeeds();

    at.write("f3.txt", "different");
    let res = ts.ucmd().args(&["f1.txt", "f3.txt"]).fails_with_code(1);
    assert!(res.stdout_str().contains("<"));
    assert!(res.stdout_str().contains(">"));
}

#[test]
fn test_sdiff_crlf_line_endings() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "hello\r\nworld\r\n");
    at.write("f2.txt", "hello\r\nworld\r\n");

    ts.ucmd().args(&["f1.txt", "f2.txt"]).succeeds();

    at.write("f3.txt", "hello\r\nthere\r\n");
    ts.ucmd().args(&["f1.txt", "f3.txt"]).fails_with_code(1);
}

#[test]
fn test_sdiff_multibyte_unicode_alignment() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "こんにちは\n世界\n");
    at.write("f2.txt", "こんばんは\n世界\n");

    let res = ts
        .ucmd()
        .args(&["--width=60", "f1.txt", "f2.txt"])
        .fails_with_code(1);
    let out = res.stdout_str();
    assert!(out.contains("こんにちは"));
    assert!(out.contains("こんばんは"));
    assert!(out.contains("世界"));
}

#[test]
fn test_sdiff_gutter_marker_column() {
    let (at, ts) = at_and_ts!();
    at.write("f1.txt", "aa\n");
    at.write("f2.txt", "bb\n");

    // With --expand-tabs and --width=40: matches GNU exact column spacing (#269)
    let res = ts
        .ucmd()
        .args(&["--width=40", "--expand-tabs", "f1.txt", "f2.txt"])
        .fails_with_code(1);
    assert_eq!(
        res.stdout_str(),
        "aa                 <\n                   >    bb\n"
    );

    // Default tabbed padding with --width=40: matches GNU exact column spacing
    let res_tabs = ts
        .ucmd()
        .args(&["--width=40", "f1.txt", "f2.txt"])
        .fails_with_code(1);
    assert_eq!(res_tabs.stdout_str(), "aa\t\t   <\n\t\t   >\tbb\n");
}
