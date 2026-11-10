// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use uutests::util::TestScenario;

use crate::TESTS_BINARY;

#[test]
fn unknown_param() {
    let ts = TestScenario::new("patch");
    ts.ucmd()
        .fails_with_code(2)
        .stderr_is("patch: utility not supported\n");

    ts.cmd(TESTS_BINARY).succeeds().stderr_str_check(|s| {
        s.starts_with("Expected utility name as second argument, got nothing.\n")
    });

    for subcmd in ["diff", "cmp", "uu-diff", "uucmp"] {
        TestScenario::new(subcmd)
            .ucmd()
            .arg("--foobar")
            .fails_with_code(2)
            .stderr_contains("unrecognized option '--foobar'");
    }
}

#[test]
fn cannot_read_files() {
    let ts = TestScenario::new("diff");
    ts.fixtures.touch("file");

    #[cfg(not(windows))]
    let error_message = "No such file or directory";
    #[cfg(windows)]
    let error_message = "The system cannot find the file specified.";
    let expected = format!(": nofile: {error_message}\n");

    for subcmd in ["diff", "cmp"] {
        ts.ccmd(subcmd)
            .args(&["nofile", "file"])
            .fails_with_code(2)
            .stderr_str_check(|s| s.ends_with(&expected));

        ts.ccmd(subcmd)
            .args(&["file", "nofile"])
            .fails_with_code(2)
            .stderr_str_check(|s| s.ends_with(&expected));
    }

    ts.ucmd()
        .args(&["nofile", "nofile"])
        .fails_with_code(2)
        .stderr_str_check(|s| s.matches(&expected).count() == 2);
}
