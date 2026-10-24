// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use diffutilslib::assert_diff_eq;

#[cfg(any(target_os = "linux", target_os = "android"))]
use assert_cmd::assert::OutputAssertExt;
use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs::File;
use std::io::Write;
use tempfile::{tempdir, NamedTempFile};

#[test]
fn no_differences() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    for option in ["", "-u", "-c", "-e"] {
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff");
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
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff");
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
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff");
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
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff");
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
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff");
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
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff");
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

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff");
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

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff");
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

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff");
    cmd.arg("-u").arg("-").arg("-");
    cmd.assert()
        .code(predicate::eq(0))
        .success()
        .stdout(predicate::str::is_empty());

    #[cfg(unix)]
    {
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff");
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
fn missing_file_as_empty() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let present = tmp_dir.path().join("present");
    let absent = tmp_dir.path().join("absent");
    std::fs::write(&present, "kiwi\nmango\n")?;

    for option in ["-N", "--new-file", "--unidirectional-new-file"] {
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff").arg(option).arg(&absent).arg(&present);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stdout("0a1,2\n> kiwi\n> mango\n");
    }

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("-N").arg(&present).arg(&absent);
    cmd.assert()
        .code(predicate::eq(1))
        .failure()
        .stdout("1,2d0\n< kiwi\n< mango\n");

    // Only a missing first operand is accepted here.
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff")
        .arg("--unidirectional-new-file")
        .arg(&present)
        .arg(&absent);
    cmd.assert().code(predicate::eq(2)).failure();

    // Without the option, a missing file is trouble.
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg(&absent).arg(&present);
    cmd.assert().code(predicate::eq(2)).failure();
    Ok(())
}

#[test]
fn binary_files() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let left = tmp_dir.path().join("left.bin");
    let right = tmp_dir.path().join("right.bin");
    std::fs::write(&left, b"header\x00\x7fpayload-one\n")?;
    std::fs::write(&right, b"header\x00\x7fpayload-two\n")?;

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg(&left).arg(&right);
    cmd.assert()
        .code(predicate::eq(1))
        .failure()
        .stderr(predicate::str::is_empty())
        .stdout(format!(
            "Binary files {} and {} differ\n",
            left.display(),
            right.display()
        ));

    // Identical binary inputs are not reported.
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg(&left).arg("-");
    cmd.write_stdin(b"header\x00\x7fpayload-one\n".to_vec());
    cmd.assert().code(predicate::eq(0)).success().stdout("");

    // --text forces a line-by-line comparison.
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("--text").arg(&left).arg(&right);
    cmd.assert()
        .code(predicate::eq(1))
        .failure()
        .stdout(predicate::str::starts_with("1c1\n"));
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn stdin_is_directory() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let dir = tmp_dir.path().join("dir");
    std::fs::create_dir(&dir)?;
    std::fs::write(dir.join("notes"), "alpha\n")?;
    let notes = tmp_dir.path().join("notes");
    std::fs::write(&notes, "beta\n")?;

    // The directory itself must be standard input, not its contents.
    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin!("diffutils"))
        .arg("diff")
        .arg("-")
        .arg(&notes)
        .stdin(File::open(&dir)?)
        .output()?;
    output
        .assert()
        .code(predicate::eq(1))
        .failure()
        .stdout("1c1\n< alpha\n---\n> beta\n");
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

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff");
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

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff");
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

// Regression for #269: the `-y` gutter marker must sit at the middle of the gutter.
#[test]
fn sdiff_gutter_marker_column() -> Result<(), Box<dyn std::error::Error>> {
    let mut file1 = NamedTempFile::new()?;
    file1.write_all("aa\n".as_bytes())?;
    let mut file2 = NamedTempFile::new()?;
    file2.write_all("bb\n".as_bytes())?;

    // --expand-tabs: padding is spaces.
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff")
        .arg("-y")
        .arg("--width=40")
        .arg("--expand-tabs")
        .arg(file1.path())
        .arg(file2.path());
    let output = cmd.output().unwrap().stdout;
    assert_eq!(
        output,
        b"aa                 <\n                   >    bb\n"
    );

    // Default (expand_tabs: false): padding goes through the tab path.
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff")
        .arg("-y")
        .arg("--width=40")
        .arg(file1.path())
        .arg(file2.path());
    let output = cmd.output().unwrap().stdout;
    assert_eq!(output, b"aa\t\t   <\n\t\t   >\tbb\n");

    Ok(())
}

#[test]
fn normal_format_single_line_ranges() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let old = tmp_dir.path().join("old");
    let new = tmp_dir.path().join("new");
    std::fs::write(&old, "north\neast\nsouth\nwest\n")?;
    std::fs::write(&new, "north\nsouth\nwest\nup\ndown\n")?;

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg(&old).arg(&new);
    cmd.assert()
        .code(predicate::eq(1))
        .stdout("2d1\n< east\n4a4,5\n> up\n> down\n");

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("/dev/null").arg(&old);
    if cfg!(unix) {
        cmd.assert()
            .code(predicate::eq(1))
            .stdout("0a1,4\n> north\n> east\n> south\n> west\n");
    }

    std::fs::write(&new, "north\neast\nsouth\nwest\nup\n")?;
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg(&old).arg(&new);
    cmd.assert().code(predicate::eq(1)).stdout("4a5\n> up\n");
    Ok(())
}

#[test]
fn ed_format_single_line_deletions() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let old = tmp_dir.path().join("old");
    let new = tmp_dir.path().join("new");
    std::fs::write(&old, "red\norange\nyellow\ngreen\nblue\n")?;
    std::fs::write(&new, "red\norange\nyellow\nblue\n")?;

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("-e").arg(&old).arg(&new);
    cmd.assert().code(predicate::eq(1)).stdout("4d\n");

    std::fs::write(&new, "red\nblue\n")?;
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("-e").arg(&old).arg(&new);
    cmd.assert().code(predicate::eq(1)).stdout("2,4d\n");
    Ok(())
}
