// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

#[cfg(not(windows))]
use std::fs::OpenOptions;
use uutests::{at_and_ts, new_ucmd};

// A file whose metadata length is 0 but which still yields bytes (/dev/zero)
// collapses the offset column to two characters, so the padding subtraction
// underflowed once the running offset reached three digits.
#[test]
#[cfg(unix)]
fn cmp_verbose_zero_length_metadata() {
    let (at, ts) = at_and_ts!();
    at.write_bytes("file", &[0xffu8; 150]);

    ts.ucmd()
        .args(&["--verbose", "/dev/zero", "file"])
        .fails_with_code(1)
        .stdout_contains("150   0 377");
}

// The offset column is as wide as the bytes left after the skips, as in GNU
// cmp: 9 bytes are compared here, so the offsets take one column.
#[test]
fn cmp_verbose_offset_width_after_skip() {
    let (at, ts) = at_and_ts!();
    at.write("a", "0123456789abcdefghijklmnopqrstuvwxyz");
    at.write("b", "0123456789abcdefghijklmnopqrstuvwxyz");

    ts.ucmd()
        .args(&["-l", "--ignore-initial=27:26", "a", "b"])
        .fails_with_code(1)
        .stdout_str_check(|s| s.starts_with("1 162 161\n2 163 162\n"))
        .stderr_str_check(|s| s.ends_with(" after byte 9\n"));
}

#[test]
fn cmp_incompatible_params() {
    new_ucmd!()
        .args(&["-l", "-s", "/etc/passwd", "/etc/group"])
        .fails_with_code(2)
        .stderr_str_check(|s| s.ends_with(": options -l and -s are incompatible\n"));
}

#[test]
fn cmp_stdin() {
    let (at, ts) = at_and_ts!();
    at.write("a", "a\n");

    ts.ucmd().arg("a").pipe_in("a\n").succeeds().no_output();

    ts.ucmd()
        .arg("a")
        .pipe_in("b\n")
        .fails_with_code(1)
        .no_stderr()
        .stdout_str_check(|s| s.ends_with(" - differ: char 1, line 1\n"));
}

#[test]
fn cmp_equal_files() {
    let (at, ts) = at_and_ts!();
    at.write("a", "a\n");
    at.write("b", "a\n");

    ts.ucmd().args(&["a", "b"]).succeeds().no_output();
}

#[test]
fn cmp_one_file_empty() {
    let (at, ts) = at_and_ts!();
    at.write("a", "a\n");
    at.touch("b");

    ts.ucmd()
        .args(&["a", "b"])
        .fails_with_code(1)
        .stderr_contains(" EOF on ")
        .stderr_str_check(|s| s.ends_with(" which is empty\n"));
}

#[test]
fn cmp_immediate_difference() {
    let (at, ts) = at_and_ts!();
    at.write("a", "abc\n");
    at.write("b", "bcd\n");

    ts.ucmd()
        .args(&["a", "b"])
        .fails_with_code(1)
        .stdout_str_check(|s| s.ends_with(" differ: char 1, line 1\n"));

    ts.ucmd()
        .args(&["-b", "a", "b"])
        .fails_with_code(1)
        .no_stderr()
        .stdout_str_check(|s| s.ends_with(" differ: byte 1, line 1 is 141 a 142 b\n"));

    ts.ucmd()
        .args(&["-l", "a", "b"])
        .fails_with_code(1)
        .stdout_only("1 141 142\n2 142 143\n3 143 144\n");

    ts.ucmd()
        .args(&["-l", "-b", "a", "b"])
        .fails_with_code(1)
        .stdout_only("1 141 a    142 b\n2 142 b    143 c\n3 143 c    144 d\n");
}

#[test]
fn cmp_newline_difference() {
    let (at, ts) = at_and_ts!();
    at.write("a", "abc\ndefg");
    at.write("b", "abc\ndef\ng");

    ts.ucmd()
        .args(&["a", "b"])
        .fails_with_code(1)
        .no_stderr()
        .stdout_str_check(|s| s.ends_with(" differ: char 8, line 2\n"));

    ts.ucmd()
        .args(&["-b", "a", "b"])
        .fails_with_code(1)
        .no_stderr()
        .stdout_str_check(|s| s.ends_with(" differ: byte 8, line 2 is 147 g  12 ^J\n"));

    ts.ucmd()
        .args(&["-l", "a", "b"])
        .fails_with_code(1)
        .stdout_str_check(|s| s.starts_with("8 147  12\n"))
        .stderr_contains(" EOF on")
        .stderr_str_check(|s| s.ends_with(" after byte 8\n"));

    ts.ucmd()
        .args(&["-b", "-l", "a", "b"])
        .fails_with_code(1)
        .stdout_str_check(|s| s.starts_with("8 147 g     12 ^J\n"))
        .stderr_contains(" EOF on")
        .stderr_str_check(|s| s.ends_with(" after byte 8\n"));
}

#[test]
fn cmp_max_bytes() {
    let (at, ts) = at_and_ts!();
    at.write("a", "abc efg ijkl\n");
    at.write("b", "abcdefghijkl\n");

    ts.ucmd()
        .args(&["-l", "-b", "-n", "3", "a", "b"])
        .succeeds()
        .no_output();

    ts.ucmd()
        .args(&["-l", "-b", "-n", "4", "a", "b"])
        .fails_with_code(1)
        .stdout_only("4  40      144 d\n");

    ts.ucmd()
        .args(&["-l", "-b", "-n", "13", "a", "b"])
        .fails_with_code(1)
        .stdout_only(" 4  40      144 d\n 8  40      150 h\n");

    ts.ucmd()
        .args(&["-l", "-b", "-n", "1K", "a", "b"])
        .fails_with_code(1)
        .stdout_only(" 4  40      144 d\n 8  40      150 h\n");
}

#[test]
fn cmp_skip_args_parsing() {
    let (at, ts) = at_and_ts!();
    at.write("a", "---abc\n");
    at.write("b", "###abc\n");

    ts.ucmd()
        .args(&["-i", "3", "a", "b"])
        .succeeds()
        .no_output();

    // A smaller positional skip does not override -i
    ts.ucmd()
        .args(&["-i", "3", "a", "b", "1", "1"])
        .succeeds()
        .no_output();

    // Single positional argument should only affect first file.
    ts.ucmd()
        .args(&["a", "b", "3"])
        .fails_with_code(1)
        .no_stderr()
        .stdout_str_check(|s| s.ends_with(" differ: char 1, line 1\n"));

    ts.ucmd().args(&["a", "b", "3", "3"]).succeeds().no_output();
}

#[test]
fn cmp_skip_uses_larger_of_option_and_positional() {
    let (at, ts) = at_and_ts!();
    at.write("a", "12345xyz\n");
    at.write("b", "abcdexyz\n");

    // Like GNU cmp, the larger of -i and the positional skip wins.
    ts.ucmd()
        .args(&["-i", "1", "a", "b", "5", "5"])
        .succeeds()
        .no_output();

    // The same applies when -i is given more than once.
    ts.ucmd()
        .args(&["-i", "5", "--ignore-initial=1", "a", "b"])
        .succeeds()
        .no_output();

    ts.ucmd()
        .args(&["-i", "1", "a", "b", "2", "2"])
        .fails_with_code(1)
        .no_stderr()
        .stdout_str_check(|s| s.ends_with(" differ: char 1, line 1\n"));
}

#[test]
fn cmp_skip_suffix_parsing() {
    let (at, ts) = at_and_ts!();
    at.write("a", &format!("{}c\n", "a".repeat(1024)));
    at.write("b", &format!("{}c\n", "b".repeat(1024)));

    ts.ucmd()
        .args(&["--ignore-initial=1K", "a", "b"])
        .succeeds()
        .no_output();
}

#[test]
fn cmp_skip() {
    let (at, ts) = at_and_ts!();
    at.write("a", "abc efg ijkl\n");
    at.write("b", "abcdefghijkl\n");

    ts.ucmd()
        .args(&["-l", "-b", "-i", "8", "a", "b"])
        .succeeds()
        .no_output();

    ts.ucmd()
        .args(&["-b", "-i", "4", "a", "b"])
        .fails_with_code(1)
        .no_stderr()
        .stdout_str_check(|s| s.ends_with(" differ: byte 4, line 1 is  40   150 h\n"));
}

#[test]
fn cmp_print_bytes_high_byte() {
    let (at, ts) = at_and_ts!();
    at.write("a", "a");
    at.write_bytes("b", &[0xC3]);

    // GNU cmp does not pad the printed bytes, whatever the second byte is.
    ts.ucmd()
        .args(&["-b", "a", "b"])
        .fails_with_code(1)
        .no_stderr()
        .stdout_str_check(|s| s.ends_with(" differ: byte 1, line 1 is 141 a 303 M-C\n"));
}

#[test]
fn cmp_binary() {
    let (at, ts) = at_and_ts!();
    let mut bytes = vec![0, 15, 31, 32, 33, 40, 64, 126, 127, 128, 129, 200, 254, 255];
    at.write_bytes("a", &bytes);
    bytes.reverse();
    at.write_bytes("b", &bytes);

    ts.ucmd()
        .args(&["-l", "-b", "a", "b"])
        .fails_with_code(1)
        .stdout_is(concat!(
            " 1   0 ^@   377 M-^?\n",
            " 2  17 ^O   376 M-~\n",
            " 3  37 ^_   310 M-H\n",
            " 4  40      201 M-^A\n",
            " 5  41 !    200 M-^@\n",
            " 6  50 (    177 ^?\n",
            " 7 100 @    176 ~\n",
            " 8 176 ~    100 @\n",
            " 9 177 ^?    50 (\n",
            "10 200 M-^@  41 !\n",
            "11 201 M-^A  40  \n",
            "12 310 M-H   37 ^_\n",
            "13 376 M-~   17 ^O\n",
            "14 377 M-^?   0 ^@\n"
        ));
}

#[test]
#[cfg(not(windows))]
fn cmp_fast_paths() {
    // Bound the runtime to a very short time that still allows for some resource
    // constraint to slow it down while also allowing very fast systems to exit as
    // early as possible.
    const MAX_TRIES: u8 = 50;

    let (at, ts) = at_and_ts!();

    // This test mimics one found in the GNU cmp test suite. It is used for
    // validating the /dev/null optimization.
    at.make_file("a")
        .set_len(14 * 1024 * 1024 * 1024 * 1024)
        .unwrap();
    at.make_file("b")
        .set_len(15 * 1024 * 1024 * 1024 * 1024)
        .unwrap();

    let dev_null = OpenOptions::new().write(true).open("/dev/null").unwrap();

    let mut child = ts
        .ucmd()
        .args(&["a", "b"])
        .set_stdout(dev_null)
        .run_no_wait();

    for tries in 0..=MAX_TRIES {
        assert!(
            tries < MAX_TRIES,
            "cmp took too long to run, /dev/null optimization probably not working"
        );
        if !child.is_alive() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    child.wait().unwrap().code_is(1);

    // Two stdins should be equal
    ts.ucmd().args(&["-", "-"]).succeeds().no_output();

    // Files with longer than block size equal segments should still report
    // the correct line number for the difference. Assumes 8KB block size (see
    // https://github.com/rust-lang/rust/blob/master/library/std/src/sys_common/io.rs),
    // create a 24KB equality.
    let mut bytes = " ".repeat(4095);
    bytes.push('\n');
    bytes.push_str(&" ".repeat(4096));
    let bytes = bytes.repeat(3);

    at.write("a", &format!("{bytes}A"));
    at.write("b", &format!("{bytes}B"));

    ts.ucmd()
        .args(&["a", "b"])
        .env("LC_ALL", "en_US")
        .fails_with_code(1)
        .stdout_str_check(|s| s.ends_with(" differ: byte 24577, line 4\n"));
}

#[test]
fn cmp_difference_term_follows_messages_locale() {
    let (at, ts) = at_and_ts!();
    at.write("a", "abc\n");
    at.write("b", "bcd\n");

    // LC_ALL overrides LC_MESSAGES, which overrides LANG; empty values are unset.
    for (variables, term) in [
        (&[][..], "char"),
        (
            &[("LC_ALL", "C"), ("LC_MESSAGES", "en_US.UTF-8")][..],
            "char",
        ),
        (&[("LC_ALL", "en_US.UTF-8"), ("LC_MESSAGES", "C")], "byte"),
        (&[("LC_MESSAGES", "POSIX"), ("LANG", "en_US.UTF-8")], "char"),
        (&[("LC_MESSAGES", "en_US.UTF-8"), ("LANG", "C")], "byte"),
        (&[("LC_ALL", ""), ("LANG", "en_US.UTF-8")], "byte"),
        (
            &[("LC_ALL", ""), ("LC_MESSAGES", ""), ("LANG", "C")],
            "char",
        ),
    ] {
        let expected = format!(" differ: {term} 1, line 1\n");
        // The command starts with a cleared environment except for the
        // LC_ALL=C default, which an empty value unsets.
        ts.ucmd()
            .env("LC_ALL", "")
            .envs(variables.iter().copied())
            .args(&["a", "b"])
            .fails_with_code(1)
            .stdout_str_check(|s| s.ends_with(&expected));
    }
}
