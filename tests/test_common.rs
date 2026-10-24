// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::NamedTempFile;

#[test]
fn unknown_param() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("patch");
    cmd.assert()
        .code(predicate::eq(2))
        .failure()
        .stderr(predicate::eq("patch: utility not supported\n"));

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.assert()
        .code(predicate::eq(0))
        .success()
        .stderr(predicate::str::starts_with(
            "Expected utility name as second argument, got nothing.\n",
        ));

    for subcmd in ["diff", "cmp", "uu-diff", "uucmp"] {
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg(subcmd);
        cmd.arg("--foobar");
        cmd.assert()
            .code(predicate::eq(2))
            .failure()
            .stderr(predicate::str::contains("unrecognized option '--foobar'"));
    }
    Ok(())
}

#[test]
fn cannot_read_files() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;

    let nofile = NamedTempFile::new()?;
    let nopath = nofile.into_temp_path();
    std::fs::remove_file(&nopath)?;

    #[cfg(not(windows))]
    let error_message = "No such file or directory";
    #[cfg(windows)]
    let error_message = "The system cannot find the file specified.";

    for subcmd in ["diff", "cmp"] {
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg(subcmd);
        cmd.arg(&nopath).arg(file.path());
        cmd.assert()
            .code(predicate::eq(2))
            .failure()
            .stderr(predicate::str::ends_with(format!(
                ": {}: {error_message}\n",
                &nopath.as_os_str().to_string_lossy()
            )));

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg(subcmd);
        cmd.arg(file.path()).arg(&nopath);
        cmd.assert()
            .code(predicate::eq(2))
            .failure()
            .stderr(predicate::str::ends_with(format!(
                ": {}: {error_message}\n",
                &nopath.as_os_str().to_string_lossy()
            )));
    }

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff");
    cmd.arg(&nopath).arg(&nopath);
    cmd.assert().code(predicate::eq(2)).failure().stderr(
        predicate::str::contains(format!(
            ": {}: {error_message}\n",
            &nopath.as_os_str().to_string_lossy()
        ))
        .count(2),
    );

    Ok(())
}
