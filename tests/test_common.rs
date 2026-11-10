// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use rstest::rstest;
use uutests::util::TestScenario;

use crate::TESTS_BINARY;

#[cfg(not(windows))]
const NOT_FOUND: &str = "No such file or directory";
#[cfg(windows)]
const NOT_FOUND: &str = "The system cannot find the file specified.";

#[test]
fn unsupported_utility() {
    TestScenario::new("patch")
        .ucmd()
        .fails_with_code(2)
        .stderr_is("patch: utility not supported\n");
}

#[test]
fn missing_utility() {
    let ts = TestScenario::new("diffutils");
    ts.cmd(TESTS_BINARY).succeeds().stderr_str_check(|s| {
        s.starts_with("Expected utility name as second argument, got nothing.\n")
    });
}

#[rstest]
fn unknown_param(#[values("diff", "cmp", "uu-diff", "uucmp")] utility: &str) {
    TestScenario::new(utility)
        .ucmd()
        .arg("--foobar")
        .fails_with_code(2)
        .stderr_contains("unrecognized option '--foobar'");
}

#[rstest]
fn cannot_read_files(
    #[values("diff", "cmp")] utility: &str,
    #[values(["nofile", "file"], ["file", "nofile"])] operands: [&str; 2],
) {
    let ts = TestScenario::new(utility);
    ts.fixtures.touch("file");

    ts.ucmd()
        .args(&operands)
        .fails_with_code(2)
        .stderr_str_check(|s| s.ends_with(&format!(": nofile: {NOT_FOUND}\n")));
}

#[test]
fn cannot_read_both_files() {
    let expected = format!(": nofile: {NOT_FOUND}\n");
    TestScenario::new("diff")
        .ucmd()
        .args(&["nofile", "nofile"])
        .fails_with_code(2)
        .stderr_str_check(|s| s.matches(&expected).count() == 2);
}
