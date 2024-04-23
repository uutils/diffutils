// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use assert_cmd::cmd::Command;
use diffutilslib::assert_diff_eq;
use predicates::prelude::*;
use std::fs::File;
use std::io::Write;
use tempfile::{tempdir, NamedTempFile};

// Integration tests for the diffutils command

#[test]
fn unknown_param() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("--foobar");
    cmd.assert()
        .code(predicate::eq(2))
        .failure()
        .stderr(predicate::str::starts_with("Unknown option: \"--foobar\""));
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

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg(&nopath).arg(file.path());
    cmd.assert()
        .code(predicate::eq(2))
        .failure()
        .stderr(predicate::str::ends_with(format!(
            ": {}: {error_message}\n",
            &nopath.as_os_str().to_string_lossy()
        )));

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg(file.path()).arg(&nopath);
    cmd.assert()
        .code(predicate::eq(2))
        .failure()
        .stderr(predicate::str::ends_with(format!(
            ": {}: {error_message}\n",
            &nopath.as_os_str().to_string_lossy()
        )));

    let mut cmd = Command::cargo_bin("diffutils")?;
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
fn compare_file_to_directory() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;

    let directory = tmp_dir.path().join("d");
    let _ = std::fs::create_dir(&directory);

    let a_path = tmp_dir.path().join("a");
    let mut a = File::create(&a_path).unwrap();
    a.write_all(b"a\n").unwrap();

    let da_path = directory.join("a");
    let mut da = File::create(&da_path).unwrap();
    da.write_all(b"da\n").unwrap();

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("-u").arg(&directory).arg(&a_path);
    cmd.assert().code(predicate::eq(1)).failure();

    let output = cmd.output().unwrap().stdout;
    assert_diff_eq!(
        output,
        format!(
            "--- {}\tTIMESTAMP\n+++ {}\tTIMESTAMP\n@@ -1 +1 @@\n-da\n+a\n",
            da_path.display(),
            a_path.display()
        )
    );

    let mut cmd = Command::cargo_bin("diffutils")?;
    cmd.arg("-u").arg(&a_path).arg(&directory);
    cmd.assert().code(predicate::eq(1)).failure();

    let output = cmd.output().unwrap().stdout;
    assert_diff_eq!(
        output,
        format!(
            "--- {}\tTIMESTAMP\n+++ {}\tTIMESTAMP\n@@ -1 +1 @@\n-a\n+da\n",
            a_path.display(),
            da_path.display()
        )
    );

    Ok(())
}
