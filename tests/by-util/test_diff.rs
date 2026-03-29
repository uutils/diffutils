// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// #[cfg(target_os = "linux")]

// spell-checker:ignore alef alefr alefx betr betx nodiff

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::{PredicateBooleanExt, predicate};
use std::{fs::File, io::Write};
use tempfile::{NamedTempFile, tempdir};
use uudiff::assert_diff_eq;
use uutests::new_ucmd;

mod diff {

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
        new_ucmd!()
            .arg("-e")
            .arg(file1.path())
            .arg(file2.path())
            .fails_with_code(2)
            .stderr_str()
            .starts_with("No newline at end of file");
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

#[cfg(test)]
mod parser {
    use ::diff::params_diff::Params;
    use diff::{clap_preparation, params_diff::FormatOutput};
    use uudiff::error::UResult;

    // use super::*;
    use std::ffi::OsString;

    fn os(s: &str) -> OsString {
        OsString::from(s)
    }

    /// Simplify call of parser, just pass a normal string like in the terminal.
    fn parse(args: &str) -> UResult<Params> {
        let opts = args
            .split(' ')
            .filter(|arg| !arg.is_empty())
            .map(OsString::from);

        let opts = clap_preparation(opts);
        let matches =
            uudiff::clap_localization::handle_clap_result(::diff::params_diff::uu_app(), opts)?;
        let params: Params = matches.try_into()?;

        Ok(params)
    }

    #[test]
    fn test_param_basics() {
        let params = Params {
            from: os("foo"),
            to: os("bar"),
            ..Default::default()
        };
        assert_eq!(params, parse("diff foo bar").unwrap());
        assert_eq!(params, parse("diff --normal foo bar").unwrap());
    }

    #[test]
    fn test_param_ed() {
        for arg in ["-e", "--ed"] {
            assert_eq!(
                Params {
                    from: os("foo"),
                    to: os("bar"),
                    format_out: FormatOutput::Ed,
                    ..Default::default()
                },
                parse(&format!("diff {arg} foo bar")).unwrap()
            );
        }
    }

    #[test]
    fn test_conflicting_output_styles() {
        for arg in [
            "-u -c",
            "-u -e",
            "-c -u",
            "-c -U42",
            "-u --normal",
            "--normal -e",
            "--context --normal",
        ] {
            assert!(parse(&format!("diff {arg} foo bar")).is_err());
        }
    }

    #[test]
    fn context_valid() {
        for arg in ["-c", "--context", "--context="] {
            assert_eq!(
                Params {
                    from: os("foo"),
                    to: os("bar"),
                    format_out: FormatOutput::Context,
                    ..Default::default()
                },
                parse(&format!("diff {arg} foo bar")).unwrap()
            );
        }

        for arg in ["-c=42", "-C42", "-C 42", "--context=42"] {
            assert_eq!(
                Params {
                    from: os("foo"),
                    to: os("bar"),
                    format_out: FormatOutput::Context,
                    n_output_lines: 42,
                    ..Default::default()
                },
                parse(&format!("diff {arg} foo bar")).unwrap()
            );
        }
    }

    /// These tests are failing as clap cannot be configured to read this
    /// possibly able to handle with: .allow_external_subcommands(true)
    #[test]
    fn context_valid_clap_limitation() {
        for arg in ["-c42", "-42c"] {
            dbg!(arg);
            assert_eq!(
                Params {
                    from: os("foo"),
                    to: os("bar"),
                    format_out: FormatOutput::Context,
                    n_output_lines: 42,
                    ..Default::default()
                },
                parse(&format!("diff {arg} foo bar")).unwrap()
            );
        }
    }

    #[test]
    fn context_invalid() {
        for arg in [
            "-c 42",
            // TODO allowed? "-c=42", works here
            // "-c=", works here, default
            "-C",
            // "-C=42", works here
            // "-C=", works here
            "--context42",
            "--context 42",
            "-42C",
        ] {
            // dbg!(&arg);
            assert!(parse(&format!("diff {arg} foo bar")).is_err());
        }
    }

    #[test]
    fn context_lines_count() {
        // clap limitation requires pre-parsing
        assert_eq!(
            Params {
                from: os("foo"),
                to: os("bar"),
                format_out: FormatOutput::Unified,
                n_output_lines: 54,
                ..Default::default()
            },
            parse("diff -u54 foo bar").unwrap()
        );

        assert_eq!(
            Params {
                from: os("foo"),
                to: os("bar"),
                format_out: FormatOutput::Unified,
                n_output_lines: 54,
                ..Default::default()
            },
            parse("diff -U54 foo bar").unwrap()
        );

        assert_eq!(
            Params {
                from: os("foo"),
                to: os("bar"),
                format_out: FormatOutput::Unified,
                n_output_lines: 54,
                ..Default::default()
            },
            parse("diff -U 54 foo bar").unwrap()
        );

        // clap limitation requires pre-parsing
        // https://github.com/clap-rs/clap/issues/6312
        assert_eq!(
            Params {
                from: os("foo"),
                to: os("bar"),
                format_out: FormatOutput::Context,
                n_output_lines: 54,
                ..Default::default()
            },
            parse("diff -c54 foo bar").unwrap()
        );
    }

    #[test]
    fn unified_valid() {
        for arg in ["-u", "--unified", "--unified="] {
            assert_eq!(
                Params {
                    from: os("foo"),
                    to: os("bar"),
                    format_out: FormatOutput::Unified,
                    ..Default::default()
                },
                parse(&format!("diff {arg} foo bar")).unwrap()
            );
        }

        for arg in ["-U42", "-U 42", "--unified=42"] {
            assert_eq!(
                Params {
                    from: os("foo"),
                    to: os("bar"),
                    format_out: FormatOutput::Unified,
                    n_output_lines: 42,
                    ..Default::default()
                },
                parse(&format!("diff {arg} foo bar")).unwrap()
            );
        }
    }

    /// These tests are failing as clap cannot be configured to read this
    /// possibly able to handle with: .allow_external_subcommands(true)
    #[test]
    fn unified_valid_clap_limitation() {
        for arg in ["-u42", "-42u"] {
            dbg!(arg);
            assert_eq!(
                Params {
                    from: os("foo"),
                    to: os("bar"),
                    format_out: FormatOutput::Unified,
                    n_output_lines: 42,
                    ..Default::default()
                },
                parse(&format!("diff {arg} foo bar")).unwrap()
            );
        }
    }

    #[test]
    fn unified_invalid() {
        for arg in [
            "-u 42",
            // "-u=42", // works here
            // "-u=",   // works here
            "-U",
            // "-U=42", // works here
            // "-U=",   // works here
            "--unified42",
            "--unified 42",
            "-42U",
        ] {
            // dbg!(&arg);
            assert!(parse(&format!("diff {arg} foo bar")).is_err());
        }
    }

    #[test]
    fn test_param_brief() {
        let params = Params {
            from: os("foo"),
            to: os("bar"),
            brief: true,
            ..Default::default()
        };
        assert_eq!(params, parse("diff -q foo bar").unwrap());
        assert_eq!(params, parse("diff --brief foo bar").unwrap());
    }

    #[test]
    fn test_param_expand_tabs() {
        let params = Params {
            from: os("foo"),
            to: os("bar"),
            expand_tabs: true,
            ..Default::default()
        };
        assert_eq!(params, parse("diff -t foo bar").unwrap());
        assert_eq!(params, parse("diff --expand-tabs foo bar").unwrap());
    }

    #[test]
    fn test_param_report_identical_files() {
        let params = Params {
            from: os("foo"),
            to: os("bar"),
            report_identical_files: true,
            ..Default::default()
        };
        assert_eq!(params, parse("diff -s foo bar").unwrap());
        assert_eq!(
            params,
            parse("diff --report-identical-files foo bar").unwrap()
        );
    }

    #[test]
    fn test_param_tabsize() {
        let mut params = Params {
            from: os("foo"),
            to: os("bar"),
            tabsize: 1,
            ..Default::default()
        };
        assert_eq!(params, parse("diff --tabsize=1 foo bar").unwrap());
        params.tabsize = 42;
        assert_eq!(params, parse("diff --tabsize=42 foo bar").unwrap());
        assert!(parse("diff --tabsize foo bar").is_err());
        assert!(parse("diff --tabsize= foo bar").is_err());
        assert!(parse("diff --tabsize=r2 foo bar").is_err());
        assert!(parse("diff --tabsize=-1 foo bar").is_err());
        assert!(parse("diff --tabsize=92233720368547758088 foo bar").is_err());
    }

    #[test]
    fn test_param_width() {
        let mut params = Params {
            from: os("foo"),
            to: os("bar"),
            width: 130,
            ..Default::default()
        };
        assert_eq!(params, parse("diff foo bar").unwrap());
        params.width = 42;
        assert_eq!(params, parse("diff -W42 foo bar").unwrap());
        assert_eq!(params, parse("diff -W 42 foo bar").unwrap());
        assert_eq!(params, parse("diff --width=42 foo bar").unwrap());
        assert_eq!(params, parse("diff --width 42 foo bar").unwrap());
        assert!(parse("diff --width foo bar").is_err());
    }

    #[test]
    fn test_double_dash() {
        let params = Params {
            from: os("-g"),
            to: os("-h"),
            ..Default::default()
        };
        assert_eq!(params, parse("diff -- -g -h").unwrap());
    }

    #[test]
    fn test_default_to_stdin() {
        let params = Params {
            from: os("foo"),
            to: os("-"),
            ..Default::default()
        };
        assert_eq!(params, parse("diff foo -").unwrap());
        assert_eq!(
            Params {
                from: os("-"),
                to: os("bar"),
                ..Default::default()
            },
            parse("diff - bar").unwrap()
        );
        assert_eq!(
            Params {
                from: os("-"),
                to: os("-"),
                ..Default::default()
            },
            parse("diff - -").unwrap()
        );
        assert!(parse("diff foo bar -").is_err());
        assert!(parse("diff - - -").is_err());
    }

    #[test]
    fn test_missing_arguments() {
        assert!(parse("diff").is_err());
        assert!(parse("diff foo").is_err());
    }

    #[test]
    fn test_unknown_argument() {
        assert!(parse("diff -g foo bar").is_err());
        assert!(parse("diff -g bar").is_err());
        assert!(parse("diff -g").is_err());
    }

    #[test]
    fn test_no_arguments() {
        assert!(parse("").is_err());
    }
}
