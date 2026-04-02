// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

// spell-checker:ignore ndefg ijkl

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::NamedTempFile;
// use uutests::new_ucmd; does not work for diffutils itself

// Integration tests for the diffutils command
mod common {

    use super::*;

    #[test]
    fn test_unknown_param() {
        // no util as argument
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stdout(predicate::str::contains("Usage: diffutils"));

        // util not recognized
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("exterminator");
        cmd.assert()
            .code(predicate::eq(2))
            .failure()
            .stderr(predicate::eq("diffutils: unknown program 'exterminator'\n"));

        for sub_cmd in ["diff", "cmp"] {
            let mut cmd = cargo_bin_cmd!("diffutils");
            cmd.arg(sub_cmd);
            cmd.arg("--foobar");
            cmd.assert()
                .code(predicate::eq(2))
                .failure()
                .stderr(predicate::str::contains("unexpected option '--foobar'"));
        }
    }

    #[test]
    fn cannot_read_files() -> Result<(), Box<dyn std::error::Error>> {
        let file = NamedTempFile::new()?;

        let no_file = NamedTempFile::new()?;
        let no_path = no_file.into_temp_path();
        std::fs::remove_file(&no_path)?;

        // #[cfg(not(windows))]
        let error_message = "No such file or directory";
        // #[cfg(windows)]
        // let error_message = "The system cannot find the file specified.";

        for sub_cmd in ["diff", "cmp"] {
            // dbg!(&sub_cmd, &no_path.as_os_str().to_string_lossy());
            let mut cmd = cargo_bin_cmd!("diffutils");
            cmd.arg(sub_cmd);
            cmd.arg(&no_path).arg(file.path());
            cmd.assert()
                .code(predicate::eq(2))
                .failure()
                .stderr(predicate::str::ends_with(format!(
                    ": {}: {error_message}\n",
                    &no_path.as_os_str().to_string_lossy()
                )));

            let mut cmd = cargo_bin_cmd!("diffutils");
            cmd.arg(sub_cmd);
            cmd.arg(file.path()).arg(&no_path);
            cmd.assert()
                .code(predicate::eq(2))
                .failure()
                .stderr(predicate::str::ends_with(format!(
                    ": {}: {error_message}\n",
                    &no_path.as_os_str().to_string_lossy()
                )));
        }

        // This requires two error messages. This is difficult to replicate
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff");
        cmd.arg(&no_path).arg(&no_path);
        cmd.assert().code(predicate::eq(2)).failure().stderr(
            predicate::str::contains(format!(
                ": {}: {error_message}\n",
                &no_path.as_os_str().to_string_lossy()
            ))
            .count(2),
        );

        Ok(())
    }
}
