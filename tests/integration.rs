// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs::File;
#[cfg(not(windows))]
use std::fs::OpenOptions;
use std::io::Write;
use tempfile::{tempdir, NamedTempFile};

// Integration tests for the diffutils command
mod common {
    use super::*;

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

        for subcmd in ["diff", "cmp"] {
            let mut cmd = cargo_bin_cmd!("diffutils");
            cmd.arg(subcmd);
            cmd.arg("--foobar");
            cmd.assert()
                .code(predicate::eq(2))
                .failure()
                .stderr(predicate::str::starts_with("Unknown option: \"--foobar\""));
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
}

mod diff {
    use diffutilslib::assert_diff_eq;

    use super::*;

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
}

mod cmp {
    use super::*;

    #[test]
    fn cmp_incompatible_params() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg("-l");
        cmd.arg("-s");
        cmd.arg("/etc/passwd").arg("/etc/group");
        cmd.assert()
            .code(predicate::eq(2))
            .failure()
            .stderr(predicate::str::ends_with(
                ": options -l and -s are incompatible\n",
            ));

        Ok(())
    }

    #[test]
    fn cmp_stdin() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let a_path = tmp_dir.path().join("a");
        let mut a = File::create(&a_path).unwrap();
        a.write_all(b"a\n").unwrap();

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg(&a_path);
        cmd.write_stdin("a\n");
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::is_empty());

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg(&a_path);
        cmd.write_stdin("b\n");
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::ends_with(" - differ: char 1, line 1\n"));

        Ok(())
    }

    #[test]
    fn cmp_equal_files() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let a_path = tmp_dir.path().join("a");
        let mut a = File::create(&a_path).unwrap();
        a.write_all(b"a\n").unwrap();

        let b_path = tmp_dir.path().join("b");
        let mut b = File::create(&b_path).unwrap();
        b.write_all(b"a\n").unwrap();

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::is_empty());

        Ok(())
    }

    #[test]
    fn cmp_one_file_empty() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let a_path = tmp_dir.path().join("a");
        let mut a = File::create(&a_path).unwrap();
        a.write_all(b"a\n").unwrap();

        let b_path = tmp_dir.path().join("b");
        let _ = File::create(&b_path).unwrap();

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stderr(predicate::str::contains(" EOF on "))
            .stderr(predicate::str::ends_with(" which is empty\n"));

        Ok(())
    }

    #[test]
    fn cmp_immediate_difference() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let a_path = tmp_dir.path().join("a");
        let mut a = File::create(&a_path).unwrap();
        a.write_all(b"abc\n").unwrap();

        let b_path = tmp_dir.path().join("b");
        let mut b = File::create(&b_path).unwrap();
        b.write_all(b"bcd\n").unwrap();

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stdout(predicate::str::ends_with(" differ: char 1, line 1\n"));

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg("-b");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::ends_with(
                " differ: byte 1, line 1 is 141 a 142 b\n",
            ));

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg("-l");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::eq("1 141 142\n2 142 143\n3 143 144\n"));

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg("-l");
        cmd.arg("-b");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::eq(
                "1 141 a    142 b\n2 142 b    143 c\n3 143 c    144 d\n",
            ));

        Ok(())
    }

    #[test]
    fn cmp_newline_difference() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let a_path = tmp_dir.path().join("a");
        let mut a = File::create(&a_path).unwrap();
        a.write_all(b"abc\ndefg").unwrap();

        let b_path = tmp_dir.path().join("b");
        let mut b = File::create(&b_path).unwrap();
        b.write_all(b"abc\ndef\ng").unwrap();

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::ends_with(" differ: char 8, line 2\n"));

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg("-b");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::ends_with(
                " differ: byte 8, line 2 is 147 g  12 ^J\n",
            ));

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg("-l");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stdout(predicate::str::starts_with("8 147  12\n"))
            .stderr(predicate::str::contains(" EOF on"))
            .stderr(predicate::str::ends_with(" after byte 8\n"));

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg("-b");
        cmd.arg("-l");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stdout(predicate::str::starts_with("8 147 g     12 ^J\n"))
            .stderr(predicate::str::contains(" EOF on"))
            .stderr(predicate::str::ends_with(" after byte 8\n"));

        Ok(())
    }

    #[test]
    fn cmp_max_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let a_path = tmp_dir.path().join("a");
        let mut a = File::create(&a_path).unwrap();
        a.write_all(b"abc efg ijkl\n").unwrap();

        let b_path = tmp_dir.path().join("b");
        let mut b = File::create(&b_path).unwrap();
        b.write_all(b"abcdefghijkl\n").unwrap();

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg("-l");
        cmd.arg("-b");
        cmd.arg("-n");
        cmd.arg("3");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::is_empty());

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg("-l");
        cmd.arg("-b");
        cmd.arg("-n");
        cmd.arg("4");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::eq("4  40      144 d\n"));

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg("-l");
        cmd.arg("-b");
        cmd.arg("-n");
        cmd.arg("13");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::eq(" 4  40      144 d\n 8  40      150 h\n"));
        Ok(())
    }

    #[test]
    fn cmp_skip_args_parsing() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let a_path = tmp_dir.path().join("a");
        let mut a = File::create(&a_path).unwrap();
        a.write_all(b"---abc\n").unwrap();

        let b_path = tmp_dir.path().join("b");
        let mut b = File::create(&b_path).unwrap();
        b.write_all(b"###abc\n").unwrap();

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg("-i");
        cmd.arg("3");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::is_empty());

        // Positional skips should be ignored
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg("-i");
        cmd.arg("3");
        cmd.arg(&a_path).arg(&b_path);
        cmd.arg("1").arg("1");
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::is_empty());

        // Single positional argument should only affect first file.
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg(&a_path).arg(&b_path);
        cmd.arg("3");
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::ends_with(" differ: char 1, line 1\n"));

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.env("LC_ALL", "C");
        cmd.arg("cmp");
        cmd.arg(&a_path).arg(&b_path);
        cmd.arg("3");
        cmd.arg("3");
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::is_empty());

        Ok(())
    }

    #[test]
    fn cmp_skip_suffix_parsing() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let a_path = tmp_dir.path().join("a");
        let mut a = File::create(&a_path).unwrap();
        writeln!(a, "{}c", "a".repeat(1024)).unwrap();
        a.flush().unwrap();

        let b_path = tmp_dir.path().join("b");
        let mut b = File::create(&b_path).unwrap();
        writeln!(b, "{}c", "b".repeat(1024)).unwrap();
        b.flush().unwrap();

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg("--ignore-initial=1K");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::is_empty());

        Ok(())
    }

    #[test]
    fn cmp_skip() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let a_path = tmp_dir.path().join("a");
        let mut a = File::create(&a_path).unwrap();
        a.write_all(b"abc efg ijkl\n").unwrap();

        let b_path = tmp_dir.path().join("b");
        let mut b = File::create(&b_path).unwrap();
        b.write_all(b"abcdefghijkl\n").unwrap();

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg("-l");
        cmd.arg("-b");
        cmd.arg("-i");
        cmd.arg("8");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::is_empty());

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg("-b");
        cmd.arg("-i");
        cmd.arg("4");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::ends_with(
                " differ: byte 4, line 1 is  40   150 h\n",
            ));

        Ok(())
    }

    #[test]
    fn cmp_binary() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mut bytes = vec![0, 15, 31, 32, 33, 40, 64, 126, 127, 128, 129, 200, 254, 255];

        let a_path = tmp_dir.path().join("a");
        let mut a = File::create(&a_path).unwrap();
        a.write_all(&bytes).unwrap();

        bytes.reverse();

        let b_path = tmp_dir.path().join("b");
        let mut b = File::create(&b_path).unwrap();
        b.write_all(&bytes).unwrap();

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg("-l");
        cmd.arg("-b");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stdout(predicate::eq(concat!(
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
            )));

        Ok(())
    }

    #[test]
    #[cfg(not(windows))]
    fn cmp_fast_paths() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        // This test mimics one found in the GNU cmp test suite. It is used for
        // validating the /dev/null optimization.
        let a_path = tmp_dir.path().join("a");
        let a = File::create(&a_path).unwrap();
        a.set_len(14 * 1024 * 1024 * 1024 * 1024).unwrap();

        let b_path = tmp_dir.path().join("b");
        let b = File::create(&b_path).unwrap();
        b.set_len(15 * 1024 * 1024 * 1024 * 1024).unwrap();

        let dev_null = OpenOptions::new().write(true).open("/dev/null").unwrap();

        let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("diffutils"))
            .arg("cmp")
            .arg(&a_path)
            .arg(&b_path)
            .stdout(dev_null)
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));

        assert_eq!(child.try_wait().unwrap().unwrap().code(), Some(1));

        // Two stdins should be equal
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg("-");
        cmd.arg("-");
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stdout(predicate::str::is_empty())
            .stderr(predicate::str::is_empty());

        // Files with longer than block size equal segments should still report
        // the correct line number for the difference. Assumes 8KB block size (see
        // https://github.com/rust-lang/rust/blob/master/library/std/src/sys_common/io.rs),
        // create a 24KB equality.
        let mut bytes = " ".repeat(4095);
        bytes.push('\n');
        bytes.push_str(&" ".repeat(4096));

        let bytes = bytes.repeat(3);
        let bytes = bytes.as_bytes();

        let a_path = tmp_dir.path().join("a");
        let mut a = File::create(&a_path).unwrap();
        a.write_all(bytes).unwrap();
        a.write_all(b"A").unwrap();

        let b_path = tmp_dir.path().join("b");
        let mut b = File::create(&b_path).unwrap();
        b.write_all(bytes).unwrap();
        b.write_all(b"B").unwrap();

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("cmp");
        cmd.arg(&a_path).arg(&b_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure()
            .stdout(predicate::str::ends_with(" differ: byte 24577, line 4\n"));

        Ok(())
    }
}

mod diff3 {
    use super::*;

    #[test]
    fn diff3_identical_files() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nline2\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\nline2\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nline2\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stdout(predicate::eq(""));

        Ok(())
    }

    #[test]
    fn diff3_with_changes() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nmodified\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\nline2\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nline2\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success();

        Ok(())
    }

    #[test]
    fn diff3_merged_format() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nmine_version\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\noriginal\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nyours_version\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-m");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure();

        Ok(())
    }

    #[test]
    fn diff3_ed_format() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nline2\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\nline2\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nmodified\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-e");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success();

        Ok(())
    }

    #[test]
    fn diff3_with_text_flag() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nline2\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\nline2\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nline2\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-a");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stdout(predicate::eq(""));

        Ok(())
    }

    #[test]
    fn diff3_with_labels() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nmine_version\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\noriginal\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nyours_version\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-m");
        cmd.arg("--label=mine_version");
        cmd.arg("--label=original");
        cmd.arg("--label=yours_version");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure();

        Ok(())
    }

    #[test]
    fn diff3_easy_only() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nmodified\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\noriginal\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\noriginal\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-3");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success();

        Ok(())
    }

    #[test]
    fn diff3_missing_file() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"content\n")?;

        let nofile = tmp_dir.path().join("nonexistent");

        #[cfg(not(windows))]
        let error_message = "No such file or directory";
        #[cfg(windows)]
        let error_message = "The system cannot find the file specified.";

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg(&mine_path).arg(&nofile).arg(&mine_path);
        cmd.assert()
            .code(predicate::eq(2))
            .failure()
            .stderr(predicate::str::contains(error_message));

        Ok(())
    }

    #[test]
    fn diff3_stdin() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\nline2\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nline2\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-").arg(&older_path).arg(&yours_path);
        cmd.write_stdin(b"line1\nline2\nline3\n");
        cmd.assert()
            .code(predicate::eq(0))
            .success();

        Ok(())
    }

    #[test]
    fn diff3_show_all_flag() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nmine_version\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\noriginal\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nyours_version\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-A");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure();

        Ok(())
    }

    #[test]
    fn diff3_three_way_conflict() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        // All three files have different content at line 2
        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nversion_a\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\noriginal\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nversion_b\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-m");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure();

        Ok(())
    }

    #[test]
    fn diff3_overlap_only_option() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        // Create files where all three differ (overlapping conflict)
        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nversion_a\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\noriginal\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nversion_b\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-x");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(1))
            .failure();

        Ok(())
    }

    #[test]
    fn diff3_easy_only_option() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        // Create files where only yours changed (easy conflict)
        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\noriginal\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\noriginal\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nmodified\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-3");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success();

        Ok(())
    }

    #[test]
    fn diff3_merged_with_overlap_only() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        // Create files where all three differ (overlapping conflict)
        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nversion_a\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\noriginal\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nversion_b\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-m");
        cmd.arg("-X");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        // -X shows only overlapping conflicts with markers, which exists in this case
        cmd.assert()
            .code(predicate::eq(1))
            .failure();

        Ok(())
    }

    #[test]
    fn diff3_ed_with_compat_i_flag() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\noriginal\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\noriginal\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nmodified\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-e");
        cmd.arg("-i");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success()
            .stdout(predicate::str::contains("w").or(predicate::str::contains("q")));

        Ok(())
    }

    #[test]
    fn diff3_ed_without_compat_i_flag() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\noriginal\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\noriginal\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nmodified\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-e");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert()
            .code(predicate::eq(0))
            .success();

        Ok(())
    }

    #[test]
    fn diff3_ed_compat_i_with_conflict() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\nmine_version\nline3\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\noriginal\nline3\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\nyours_version\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-e");
        cmd.arg("-i");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        // Even with conflicts, -e -i should produce a valid ed script with w and q
        cmd.assert()
            .code(predicate::eq(1))
            .failure();

        Ok(())
    }

    #[test]
    fn diff3_identical_large_files() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        // Create identical large files (10,000 lines each)
        let mut large_content = String::new();
        for i in 0..10000 {
            large_content.push_str(&format!("line {}\n", i));
        }
        let content_bytes = large_content.as_bytes();

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(content_bytes)?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(content_bytes)?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(content_bytes)?;

        // With identical files, output should be empty
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert().code(0).stdout("");

        Ok(())
    }

    #[test]
    fn diff3_large_file_with_single_line_change() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        // Create large file with single change in both mine and yours differently
        let mut older_content = String::new();
        let mut mine_content = String::new();
        let mut yours_content = String::new();
        for i in 0..10000 {
            let line = format!("line {}\n", i);
            older_content.push_str(&line);
            
            if i == 5000 {
                mine_content.push_str("line 5000 MINE\n");
                yours_content.push_str("line 5000 YOURS\n");
            } else {
                mine_content.push_str(&line);
                yours_content.push_str(&line);
            }
        }

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(mine_content.as_bytes())?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(older_content.as_bytes())?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(yours_content.as_bytes())?;

        // Should detect the conflict in line 5000
        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        cmd.assert().code(1);  // Has conflict

        Ok(())
    }

    #[test]
    fn diff3_large_file_merged_format() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        // Create large files with changes in different sections
        let mut mine_content = String::new();
        let mut older_content = String::new();
        let mut yours_content = String::new();

        for i in 0..5000 {
            let line = format!("line {}\n", i);
            older_content.push_str(&line);
            
            if i < 2500 {
                mine_content.push_str(&line);
                yours_content.push_str(&line);
            } else if i < 3750 {
                mine_content.push_str(&format!("mine {}\n", i));
                yours_content.push_str(&line);
            } else {
                mine_content.push_str(&line);
                yours_content.push_str(&format!("yours {}\n", i));
            }
        }

        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(mine_content.as_bytes())?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(older_content.as_bytes())?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(yours_content.as_bytes())?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-m");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        // Should produce merged output with some conflicts
        cmd.assert().code(1);

        Ok(())
    }

    #[test]
    fn diff3_binary_files_with_null_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        // Create binary files with null bytes (like image files)
        let mine_path = tmp_dir.path().join("mine.bin");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"GIF89a\x00\x10\x00\x10\xFF\xFF\xFF")?;

        let older_path = tmp_dir.path().join("older.bin");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"GIF89a\x00\x10\x00\x10\xFF\xFF\xFF")?;

        let yours_path = tmp_dir.path().join("yours.bin");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"PNG\x89\x50\x4E\x47\x0D\x0A\x1A\x0A")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        // Binary files should be detected and reported as different
        cmd.assert().code(1);

        Ok(())
    }

    #[test]
    fn diff3_identical_binary_files() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        let content = b"GIF89a\x00\x10\x00\x10\xFF\xFF\xFF";

        let mine_path = tmp_dir.path().join("mine.bin");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(content)?;

        let older_path = tmp_dir.path().join("older.bin");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(content)?;

        let yours_path = tmp_dir.path().join("yours.bin");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(content)?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        // Identical binary files should produce no output
        cmd.assert().code(0).stdout("");

        Ok(())
    }

    #[test]
    fn diff3_binary_files_with_text_flag() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = tempdir()?;

        // Create files with null bytes but use --text to force text processing
        let mine_path = tmp_dir.path().join("mine");
        let mut mine_file = File::create(&mine_path)?;
        mine_file.write_all(b"line1\x00\nline2\n")?;

        let older_path = tmp_dir.path().join("older");
        let mut older_file = File::create(&older_path)?;
        older_file.write_all(b"line1\x00\nline2\n")?;

        let yours_path = tmp_dir.path().join("yours");
        let mut yours_file = File::create(&yours_path)?;
        yours_file.write_all(b"line1\x00\nline3\n")?;

        let mut cmd = cargo_bin_cmd!("diffutils");
        cmd.arg("diff3");
        cmd.arg("-a");  // --text flag to force text mode
        cmd.arg(&mine_path).arg(&older_path).arg(&yours_path);
        // With --text flag, should process as text despite null byte
        // The output should show differences but not report as "Binary files differ"
        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Should not contain "Binary files differ" message
        assert!(!stdout.contains("Binary files differ"), 
                "Should not report binary when --text flag is used");
        // Should have changes detected (exit code 0 or 1 depending on conflicts)
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1),
                "Exit code should indicate success or conflicts");

        Ok(())
    }
}



