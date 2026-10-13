// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use diffutilslib::assert_diff_eq;

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
fn short_option_clusters_match_split_options() -> Result<(), Box<dyn std::error::Error>> {
    let mut left = NamedTempFile::new()?;
    left.write_all(b"a\tb\n")?;
    let mut right = NamedTempFile::new()?;
    right.write_all(b"a b\n")?;

    let equivalent = |cluster: &[&str],
                      split: &[&str],
                      other: &std::path::Path|
     -> Result<std::process::Output, Box<dyn std::error::Error>> {
        let run = |options: &[&str]| {
            cargo_bin_cmd!("diffutils")
                .arg("diff")
                .args(options)
                .arg(left.path())
                .arg(other)
                .output()
        };
        let clustered = run(cluster)?;
        let separate = run(split)?;
        assert_eq!(
            clustered.status.code(),
            separate.status.code(),
            "{cluster:?}"
        );
        assert_eq!(clustered.stdout, separate.stdout, "{cluster:?}");
        assert_eq!(clustered.stderr, separate.stderr, "{cluster:?}");
        Ok(clustered)
    };

    for (cluster, split, expected_code) in [
        (&["-cc"][..], &["-c", "-c"][..], Some(1)),
        (&["-ee"], &["-e", "-e"], Some(1)),
        (&["-qq"], &["-q", "-q"], Some(1)),
        (&["-ss"], &["-s", "-s"], Some(1)),
        (&["-tt"], &["-t", "-t"], Some(1)),
        (&["-uu"], &["-u", "-u"], Some(1)),
        (&["-cqt"], &["-c", "-q", "-t"], Some(1)),
        (&["-eqs"], &["-e", "-q", "-s"], Some(1)),
        (&["-qst"], &["-q", "-s", "-t"], Some(1)),
        (&["-uqt"], &["-u", "-q", "-t"], Some(1)),
        (&["-cu"], &["-c", "-u"], Some(2)),
        (&["-uc"], &["-u", "-c"], Some(2)),
        (&["-eu"], &["-e", "-u"], Some(2)),
        (&["-c", "-ut"], &["-c", "-u", "-t"], Some(2)),
        (&["-ut", "-c"], &["-u", "-t", "-c"], Some(2)),
    ] {
        let clustered = equivalent(cluster, split, right.path())?;
        assert_eq!(clustered.status.code(), expected_code, "{cluster:?}");
    }

    let mut same = NamedTempFile::new()?;
    same.write_all(b"a\tb\n")?;
    let identical = format!(
        "Files {} and {} are identical\n",
        left.path().to_string_lossy(),
        same.path().to_string_lossy()
    );
    for (cluster, split) in [
        (&["-ss"][..], &["-s", "-s"][..]),
        (&["-qs"], &["-q", "-s"]),
        (&["-ys"], &["-y", "-s"]),
    ] {
        let clustered = equivalent(cluster, split, same.path())?;
        assert_eq!(clustered.status.code(), Some(0), "{cluster:?}");
        assert!(
            clustered.stdout.ends_with(identical.as_bytes()),
            "{cluster:?}"
        );
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
