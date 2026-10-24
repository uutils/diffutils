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

    // -q keeps its usual wording.
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("-q").arg(&left).arg(&right);
    cmd.assert()
        .code(predicate::eq(1))
        .failure()
        .stderr(predicate::str::is_empty())
        .stdout(format!(
            "Files {} and {} differ\n",
            left.display(),
            right.display()
        ));

    // -a and --text force a line-by-line comparison.
    for option in ["-a", "--text"] {
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff").arg(option).arg(&left).arg(&right);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stdout(predicate::str::starts_with("1c1\n"));
    }
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
fn color() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let old = tmp_dir.path().join("old");
    let new = tmp_dir.path().join("new");
    std::fs::write(&old, "sun\nrain\n")?;
    std::fs::write(&new, "sun\nsnow\n")?;

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("--color=always").arg(&old).arg(&new);
    cmd.assert()
        .code(predicate::eq(1))
        .stdout("\x1b[36m2c2\x1b[0m\n\x1b[31m< rain\x1b[0m\n---\n\x1b[32m> snow\x1b[0m\n");

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff")
        .arg("--color=always")
        .arg("--palette=de=4:ad=7")
        .arg(&old)
        .arg(&new);
    cmd.assert()
        .code(predicate::eq(1))
        .stdout("\x1b[36m2c2\x1b[0m\n\x1b[4m< rain\x1b[0m\n---\n\x1b[7m> snow\x1b[0m\n");

    // Standard output is not a terminal here, and TERM=dumb never gets colors.
    for args in [&["--color"][..], &["--color=auto", "---presume-output-tty"]] {
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("TERM", "dumb")
            .arg("diff")
            .args(args)
            .arg(&old)
            .arg(&new);
        cmd.assert()
            .code(predicate::eq(1))
            .stdout("2c2\n< rain\n---\n> snow\n");
    }

    // An invalid palette is reported and disables colors.
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff")
        .arg("--color=always")
        .arg("--palette=up=1")
        .arg(&old)
        .arg(&new);
    cmd.assert()
        .code(predicate::eq(1))
        .stdout("2c2\n< rain\n---\n> snow\n")
        .stderr(predicate::str::contains("unrecognized prefix: up"));

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("--color=sometimes").arg(&old).arg(&new);
    cmd.assert()
        .code(predicate::eq(2))
        .stderr(predicate::str::contains("invalid color 'sometimes'"));

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("--colour").arg(&old).arg(&new);
    cmd.assert()
        .code(predicate::eq(2))
        .stderr(predicate::str::contains("unrecognized option '--colour'"));

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff")
        .arg("--palette")
        .arg("ln=1")
        .arg("--color=always")
        .arg(&old)
        .arg(&new);
    cmd.assert()
        .code(predicate::eq(1))
        .stdout("\x1b[1m2c2\x1b[0m\n\x1b[31m< rain\x1b[0m\n---\n\x1b[32m> snow\x1b[0m\n");
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn write_error() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let old = tmp_dir.path().join("old");
    let new = tmp_dir.path().join("new");
    std::fs::write(&old, "left\n")?;
    std::fs::write(&new, "right\n")?;

    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin!("diffutils"))
        .arg("diff")
        .arg(&old)
        .arg(&new)
        .stdout(File::create("/dev/full")?)
        .output()?;
    output
        .assert()
        .code(predicate::eq(2))
        .stderr("diff: standard output: No space left on device\n");
    Ok(())
}

// TZ is not honored on Windows.
#[cfg(unix)]
#[test]
fn context_timestamp_follows_time_locale() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let old = tmp_dir.path().join("old");
    let new = tmp_dir.path().join("new");
    std::fs::write(&old, "left\n")?;
    std::fs::write(&new, "right\n")?;
    // 2009-02-13 23:31:30.25 UTC
    let mtime = std::time::UNIX_EPOCH + std::time::Duration::from_millis(1_234_567_890_250);
    for path in [&old, &new] {
        File::options()
            .write(true)
            .open(path)?
            .set_modified(mtime)?;
    }

    let traditional = "Fri Feb 13 23:31:30 2009";
    let iso = "2009-02-13 23:31:30.250000000 +0000";
    // LC_ALL overrides LC_TIME, which overrides LANG; empty values are unset.
    for (variables, timestamp) in [
        (&[("LC_ALL", "C")][..], traditional),
        (
            &[("LC_ALL", "POSIX"), ("LC_TIME", "en_US.UTF-8")],
            traditional,
        ),
        (&[("LC_ALL", "en_US.UTF-8"), ("LC_TIME", "C")], iso),
        (&[("LC_TIME", "C"), ("LANG", "en_US.UTF-8")], traditional),
        (&[("LC_TIME", "en_US.UTF-8"), ("LANG", "C")], iso),
        (
            &[("LC_ALL", ""), ("LC_TIME", ""), ("LANG", "POSIX")],
            traditional,
        ),
        (&[("LANG", "fr_FR.UTF-8")], iso),
    ] {
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("TZ", "UTC0")
            .env_remove("LC_ALL")
            .env_remove("LC_TIME")
            .env_remove("LANG")
            .envs(variables.iter().copied())
            .arg("diff")
            .arg("-c")
            .arg(&old)
            .arg(&new);
        cmd.assert()
            .code(predicate::eq(1))
            .stdout(predicate::str::starts_with(format!(
                "*** {}\t{timestamp}\n--- {}\t{timestamp}\n",
                old.display(),
                new.display()
            )));
    }
    Ok(())
}

#[test]
fn compare_directories() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let left = tmp_dir.path().join("left");
    let right = tmp_dir.path().join("right");
    for dir in [&left, &right] {
        std::fs::create_dir_all(dir.join("nested/inner"))?;
        std::fs::write(dir.join("equal"), "same\n")?;
    }
    std::fs::write(left.join("changed"), "old\n")?;
    std::fs::write(right.join("changed"), "new\n")?;
    std::fs::write(left.join("nested/inner/deep"), "one\n")?;
    std::fs::write(right.join("nested/inner/deep"), "two\n")?;
    std::fs::write(left.join("gone"), "bye\n")?;
    std::fs::write(right.join("added"), "hi\n")?;
    std::fs::write(left.join("kind"), "file\n")?;
    std::fs::create_dir(right.join("kind"))?;
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    // Entry names are joined to the directories with the native separator.
    let s = std::path::MAIN_SEPARATOR;

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg(left).arg(right);
    cmd.assert().code(predicate::eq(1)).stdout(format!(
        "Only in {right}: added\n\
         diff {left}{s}changed {right}{s}changed\n1c1\n< old\n---\n> new\n\
         Only in {left}: gone\n\
         File {left}{s}kind is a regular file while file {right}{s}kind is a directory\n\
         Common subdirectories: {left}{s}nested and {right}{s}nested\n"
    ));

    // Options are repeated as given in the headers, and trailing slashes
    // do not end up in the compared paths.
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff")
        .arg("-r")
        .arg("-q")
        .arg(format!("{left}/"))
        .arg(right);
    cmd.assert().code(predicate::eq(1)).stdout(format!(
        "Only in {right}: added\n\
         Files {left}{s}changed and {right}{s}changed differ\n\
         Only in {left}/: gone\n\
         File {left}{s}kind is a regular file while file {right}{s}kind is a directory\n\
         Files {left}{s}nested{s}inner{s}deep and {right}{s}nested{s}inner{s}deep differ\n"
    ));

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff")
        .arg("-N")
        .arg("-r")
        .arg("-e")
        .arg(left)
        .arg(right);
    cmd.assert().code(predicate::eq(1)).stdout(format!(
        "diff -N -r -e {left}{s}added {right}{s}added\n0a\nhi\n.\n\
         diff -N -r -e {left}{s}changed {right}{s}changed\n1c\nnew\n.\n\
         diff -N -r -e {left}{s}gone {right}{s}gone\n1d\n\
         File {left}{s}kind is a regular file while file {right}{s}kind is a directory\n\
         diff -N -r -e {left}{s}nested{s}inner{s}deep {right}{s}nested{s}inner{s}deep\n1c\ntwo\n.\n"
    ));

    // -s reports the identical files, but a directory is not compared with
    // itself.
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("-r").arg("-s").arg(left).arg(left);
    cmd.assert().code(predicate::eq(0)).stdout("");

    let copy = tmp_dir.path().join("copy");
    std::fs::create_dir_all(copy.join("nested/inner"))?;
    std::fs::write(copy.join("changed"), "old\n")?;
    std::fs::write(copy.join("equal"), "same\n")?;
    std::fs::write(copy.join("nested/inner/deep"), "one\n")?;
    let copy = copy.to_str().unwrap();
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("-r").arg("-s").arg(left).arg(copy);
    cmd.assert().code(predicate::eq(1)).stdout(format!(
        "Files {left}{s}changed and {copy}{s}changed are identical\n\
         Files {left}{s}equal and {copy}{s}equal are identical\n\
         Only in {left}: gone\n\
         Only in {left}: kind\n\
         Files {left}{s}nested{s}inner{s}deep and {copy}{s}nested{s}inner{s}deep are identical\n"
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn compare_directories_special_entries() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    for dir in ["one", "two"] {
        std::fs::create_dir_all(tmp_dir.path().join(dir).join("tools/bin"))?;
        // Links to the same directory are not compared.
        std::os::unix::fs::symlink("..", tmp_dir.path().join(dir).join("parent"))?;
        let status = std::process::Command::new("mkfifo")
            .arg(tmp_dir.path().join(dir).join("queue"))
            .status()?;
        assert!(status.success());
    }
    std::fs::write(tmp_dir.path().join("one/tools/bin/run"), "go\n")?;
    std::fs::remove_dir_all(tmp_dir.path().join("two/tools"))?;

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.current_dir(tmp_dir.path())
        .arg("diff")
        .arg("-N")
        .arg("one")
        .arg("two");
    cmd.assert().code(predicate::eq(1)).stdout(
        "File one/queue is a fifo while file two/queue is a fifo\n\
         Common subdirectories: one/tools and two/tools\n",
    );

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.current_dir(tmp_dir.path())
        .arg("diff")
        .arg("-r")
        .arg("-N")
        .arg("one")
        .arg("two");
    cmd.assert().code(predicate::eq(1)).stdout(
        "File one/queue is a fifo while file two/queue is a fifo\n\
         diff -r -N one/tools/bin/run two/tools/bin/run\n1d0\n< go\n",
    );

    // A link back to a directory being compared is reported.
    std::os::unix::fs::symlink("..", tmp_dir.path().join("one/tools/again"))?;
    std::fs::create_dir(tmp_dir.path().join("two/tools"))?;
    std::os::unix::fs::symlink("..", tmp_dir.path().join("two/tools/again"))?;
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.current_dir(tmp_dir.path())
        .arg("diff")
        .arg("-r")
        .arg("one")
        .arg("two");
    cmd.assert()
        .code(predicate::eq(2))
        .stdout(predicate::str::contains("Only in one/tools: bin\n"))
        .stderr("diff: one/tools/again: recursive directory loop\n");
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

#[test]
fn function_lines() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let old = tmp_dir.path().join("old.c");
    let new = tmp_dir.path().join("new.c");
    std::fs::write(
        &old,
        "int zero;\nstatic int twice(int n)\n{\n  int r;\n  r = n * 2;\n  return r;\n}\n",
    )?;
    std::fs::write(
        &new,
        "int zero;\nstatic int twice(int n)\n{\n  int r;\n  r = n + n;\n  return r;\n}\n",
    )?;

    // -p defaults to the context format and shows the enclosing C function.
    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff").arg("-p").arg("-C1").arg(&old).arg(&new);
    cmd.assert()
        .code(predicate::eq(1))
        .stdout(predicate::str::contains(
            "\n*************** static int twice(int n)\n*** 4,6 ****\n",
        ));

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff")
        .arg("-U0")
        .arg("-F")
        .arg("^int")
        .arg(&old)
        .arg(&new);
    cmd.assert()
        .code(predicate::eq(1))
        .stdout(predicate::str::ends_with(
            "@@ -5 +5 @@ int zero;\n-  r = n * 2;\n+  r = n + n;\n",
        ));

    let mut cmd = cargo_bin_cmd!("diffutils");
    cmd.arg("diff")
        .arg("-u")
        .arg("-F")
        .arg("\\(")
        .arg(&old)
        .arg(&new);
    cmd.assert()
        .code(predicate::eq(2))
        .stderr("'\\(': Unmatched ( or \\(\n");
    Ok(())
}
