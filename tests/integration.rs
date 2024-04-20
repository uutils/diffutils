// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use assert_cmd::cmd::Command;
use diffutilslib::assert_diff_eq;
use predicates::prelude::*;
use std::fs::File;
use std::io::Write;
use tempfile::NamedTempFile;

// Integration tests for the diffutils command

#[test]
fn unknown_param() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("--foobar");
    cmd.assert()
        .code(predicate::eq(2))
        .failure()
        .stderr(predicate::str::starts_with("Usage: "));
    Ok(())
}

#[test]
fn cannot_read_files() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("foo.txt").arg(file.path());
    cmd.assert()
        .code(predicate::eq(2))
        .failure()
        .stderr(predicate::str::starts_with("Failed to read from-file"));

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg(file.path()).arg("foo.txt");
    cmd.assert()
        .code(predicate::eq(2))
        .failure()
        .stderr(predicate::str::starts_with("Failed to read to-file"));

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("foo.txt").arg("foo.txt");
    cmd.assert()
        .code(predicate::eq(2))
        .failure()
        .stderr(predicate::str::starts_with("Failed to read from-file"));

    Ok(())
}

#[test]
fn no_differences() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    for option in ["", "-u", "-c", "-e"] {
        let mut cmd = Command::cargo_bin("diffutils")?;
        if !option.is_empty() {
            cmd.arg(option);
        }
        cmd.arg(file.path()).arg(file.path());
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stdout(predicate::str::is_empty());
    }
    Ok(())
}

#[test]
fn no_differences_report_identical_files() -> Result<(), Box<dyn std::error::Error>> {
    // same file
    let mut file1 = NamedTempFile::new()?;
    file1.write_all("foo\n".as_bytes())?;
    for option in ["", "-u", "-c", "-e"] {
        let mut cmd = Command::cargo_bin("diffutils")?;
        if !option.is_empty() {
            cmd.arg(option);
        }
        cmd.arg("-s").arg(file1.path()).arg(file1.path());
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stdout(predicate::eq(format!(
                "Files {} and {} are identical\n",
                file1.path().to_string_lossy(),
                file1.path().to_string_lossy(),
            )));
    }
    // two files with the same content
    let mut file2 = NamedTempFile::new()?;
    file2.write_all("foo\n".as_bytes())?;
    for option in ["", "-u", "-c", "-e"] {
        let mut cmd = Command::cargo_bin("diffutils")?;
        if !option.is_empty() {
            cmd.arg(option);
        }
        cmd.arg("-s").arg(file1.path()).arg(file2.path());
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stdout(predicate::eq(format!(
                "Files {} and {} are identical\n",
                file1.path().to_string_lossy(),
                file2.path().to_string_lossy(),
            )));
    }
    Ok(())
}

#[test]
fn differences() -> Result<(), Box<dyn std::error::Error>> {
    let mut file1 = NamedTempFile::new()?;
    file1.write_all("foo\n".as_bytes())?;
    let mut file2 = NamedTempFile::new()?;
    file2.write_all("bar\n".as_bytes())?;
    for option in ["", "-u", "-c", "-e"] {
        let mut cmd = Command::cargo_bin("diffutils")?;
        if !option.is_empty() {
            cmd.arg(option);
        }
        cmd.arg(file1.path()).arg(file2.path());
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stdout(predicate::str::is_empty().not());
    }
    Ok(())
}

#[test]
fn differences_brief() -> Result<(), Box<dyn std::error::Error>> {
    let mut file1 = NamedTempFile::new()?;
    file1.write_all("foo\n".as_bytes())?;
    let mut file2 = NamedTempFile::new()?;
    file2.write_all("bar\n".as_bytes())?;
    for option in ["", "-u", "-c", "-e"] {
        let mut cmd = Command::cargo_bin("diffutils")?;
        if !option.is_empty() {
            cmd.arg(option);
        }
        cmd.arg("-q").arg(file1.path()).arg(file2.path());
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stdout(predicate::eq(format!(
                "Files {} and {} differ\n",
                file1.path().to_string_lossy(),
                file2.path().to_string_lossy()
            )));
    }
    Ok(())
}

#[test]
fn missing_newline() -> Result<(), Box<dyn std::error::Error>> {
    let mut file1 = NamedTempFile::new()?;
    file1.write_all("foo".as_bytes())?;
    let mut file2 = NamedTempFile::new()?;
    file2.write_all("bar".as_bytes())?;
    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("-e").arg(file1.path()).arg(file2.path());
    cmd.assert()
        .code(predicate::eq(2))
        .failure()
        .stderr(predicate::str::starts_with("No newline at end of file"));
    Ok(())
}

#[test]
fn read_from_stdin() -> Result<(), Box<dyn std::error::Error>> {
    let mut file1 = NamedTempFile::new()?;
    file1.write_all("foo\n".as_bytes())?;
    let mut file2 = NamedTempFile::new()?;
    file2.write_all("bar\n".as_bytes())?;

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("-u")
        .arg(file1.path())
        .arg("-")
        .write_stdin("bar\n");
    cmd.assert().code(predicate::eq(1)).failure();

    let output = cmd.output().unwrap().stdout;
    assert_diff_eq!(
        output,
        format!(
            "--- {}\tTIMESTAMP\n+++ -\tTIMESTAMP\n@@ -1 +1 @@\n-foo\n+bar\n",
            file1.path().to_string_lossy()
        )
    );

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("-u")
        .arg("-")
        .arg(file2.path())
        .write_stdin("foo\n");
    cmd.assert().code(predicate::eq(1)).failure();

    let output = cmd.output().unwrap().stdout;
    assert_diff_eq!(
        output,
        format!(
            "--- -\tTIMESTAMP\n+++ {}\tTIMESTAMP\n@@ -1 +1 @@\n-foo\n+bar\n",
            file2.path().to_string_lossy()
        )
    );

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("-u").arg("-").arg("-");
    cmd.assert()
        .code(predicate::eq(0))
        .success()
        .stdout(predicate::str::is_empty());

    #[cfg(unix)]
    {
        let mut cmd = Command::cargo_bin("diffutils")?;
        cmd.arg("-u")
            .arg(file1.path())
            .arg("/dev/stdin")
            .write_stdin("bar\n");
        cmd.assert().code(predicate::eq(1)).failure();

        let output = cmd.output().unwrap().stdout;
        assert_diff_eq!(
            output,
            format!(
                "--- {}\tTIMESTAMP\n+++ /dev/stdin\tTIMESTAMP\n@@ -1 +1 @@\n-foo\n+bar\n",
                file1.path().to_string_lossy()
            )
        );
    }

    Ok(())
}

#[test]
fn read_from_directory() -> Result<(), Box<dyn std::error::Error>> {
    let target = "target/integration";
    let _ = std::fs::create_dir(target);
    let directory = &format!("{target}/d");
    let _ = std::fs::create_dir(directory);
    let mut a = File::create(&format!("{target}/a")).unwrap();
    a.write_all(b"a\n").unwrap();
    let mut da = File::create(&format!("{directory}/a")).unwrap();
    da.write_all(b"da\n").unwrap();

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("-u")
        .arg(&format!("{target}/d"))
        .arg(&format!("{target}/a"));
    cmd.assert().code(predicate::eq(1)).failure();

    let output = cmd.output().unwrap().stdout;
    assert_diff_eq!(
        output,
        format!(
            "--- {}/d/a\tTIMESTAMP\n+++ {}/a\tTIMESTAMP\n@@ -1 +1 @@\n-da\n+a\n",
            target, target
        )
    );

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("-u")
        .arg(&format!("{target}/a"))
        .arg(&format!("{target}/d"));
    cmd.assert().code(predicate::eq(1)).failure();

    let output = cmd.output().unwrap().stdout;
    assert_diff_eq!(
        output,
        format!(
            "--- {}/a\tTIMESTAMP\n+++ {}/d/a\tTIMESTAMP\n@@ -1 +1 @@\n-a\n+da\n",
            target, target
        )
    );

    Ok(())
}
