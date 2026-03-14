// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Common benchmark utilities for uutils coreutils
//!
//! This module provides shared functionality for benchmarking utilities,
//! including test data generation and binary execution helpers.

use std::ffi::OsString;

/// Converts a String to a Vec which can be used as args \
/// to pass to the utilities, e.g. "diff file_a file_b -w 150".
///
/// # Returns
/// A vec OsString which can be used instead of ArgsOs.
pub fn str_to_args(args: &str) -> Vec<OsString> {
    let s: Vec<OsString> = args
        .split(" ")
        .filter(|s| !s.is_empty())
        .map(OsString::from)
        .collect();

    s
}

pub mod prepare_bench {
    use std::{
        fs::File,
        io::{BufWriter, Write},
        path::Path,
    };

    use rand::RngExt;
    use tempfile::TempDir;

    /// When a file is changed to be different, a char is inserted.
    const CHANGE_INDICATION_CHAR: u8 = b'#';
    // const FILE_SIZES_IN_KILO_BYTES: [u64; 2] = [100, 1 * 1000];

    // file lines and .txt will be added
    const FROM_FILE: &str = "from_file";
    const TO_FILE: &str = "to_file";
    const LINE_LENGTH: usize = 60;

    #[derive(Debug, Default)]
    pub struct FilePair {
        pub from: String,
        pub to: String,
        pub size_bytes: u64,
    }

    /// Contains test data (file names) which only needs to be created once.
    #[derive(Debug, Default)]
    pub struct BenchContext {
        /// Optional TempDir directory. When set, the dir is of no relevance.
        pub tmp_dir: Option<TempDir>,
        /// Directory path if TempDir is not set.
        pub dir: String,
        /// list of files in different sizes
        pub files_equal: Vec<FilePair>,
        /// list of files in different sizes
        pub files_different: Vec<FilePair>,
    }

    impl BenchContext {
        pub fn get_path(&self) -> &Path {
            match &self.tmp_dir {
                Some(tmp) => tmp.path(),
                None => Path::new(&self.dir),
            }
        }

        pub fn get_files_equal_kb(&self, kb: u64) -> Option<&FilePair> {
            self.get_files_equal(kb * 1000)
        }

        pub fn get_files_equal(&self, bytes: u64) -> Option<&FilePair> {
            let p = self.files_equal.iter().find(|f| f.size_bytes == bytes)?;
            Some(p)
        }

        pub fn get_files_different_kb(&self, kb: u64) -> Option<&FilePair> {
            self.get_files_different(kb * 1000)
        }

        pub fn get_files_different(&self, bytes: u64) -> Option<&FilePair> {
            let p = self
                .files_different
                .iter()
                .find(|f| f.size_bytes == bytes)?;
            Some(p)
        }
    }

    /// Generates two test files for comparison with <bytes> size.
    ///
    /// # Params
    /// * dir: the directory where the files are created (TempDir suggested)
    /// * bytes: the number of bytes the files will be long (exactly)
    /// * num_difference: the number of differences inserted in the diff file
    /// * id: added to the file names to differentiate for different tests
    ///
    /// # Returns
    /// (from_file_name, to_file_name): Two files of the specified size in bytes.
    ///
    /// Each line consists of 10 words with 5 letters, giving a line length of 60 bytes.
    /// If num_differences is set, '#' will be inserted between the first two words of a line,
    /// evenly spaced in the file. 1 will add the change in the last line, so the comparison takes longest.
    pub fn generate_test_files_bytes(
        dir: &Path,
        bytes: u64,
        num_differences: u64,
        id: &str,
    ) -> std::io::Result<FilePair> {
        let id = if id.is_empty() {
            "".to_string()
        } else {
            format!("{id}_")
        };
        let f1 = format!("{id}{FROM_FILE}_{bytes}.txt");
        let f2 = format!("{id}{TO_FILE}_{bytes}.txt");
        let from_path = dir.join(f1);
        let to_path = dir.join(f2);

        generate_file_bytes(&from_path, &to_path, bytes, num_differences)?;

        Ok(FilePair {
            from: from_path.to_string_lossy().to_string(),
            to: to_path.to_string_lossy().to_string(),
            size_bytes: bytes,
        })
    }

    /// Generates two test files for comparison with <bytes> size.
    ///
    /// # Returns
    /// Ok when the files were created.
    ///
    /// Like [generate_test_files_bytes] with specified file names. \
    /// The function must generate two files at once to quickly create
    /// files with minimal differences.
    pub fn generate_file_bytes(
        from_name: &Path,
        to_name: &Path,
        bytes: u64,
        num_differences: u64,
    ) -> std::io::Result<()> {
        let file_from = File::create(from_name)?;
        let file_to = File::create(to_name)?;
        // for int division, lines will be smaller than requested bytes
        let n_lines = bytes / LINE_LENGTH as u64;
        let change_every_n_lines = if num_differences == 0 {
            0
        } else {
            let c = n_lines / num_differences;
            if c == 0 {
                1
            } else {
                c
            }
        };
        // Use a larger 128KB buffer for massive files
        let mut writer_from = BufWriter::with_capacity(128 * 1024, file_from);
        let mut writer_to = BufWriter::with_capacity(128 * 1024, file_to);
        let mut rng = rand::rng();

        // Each line: (5 chars * 10 words) + 9 spaces + 1 newline = 60 bytes
        let mut line_buffer = [b' '; 60];
        line_buffer[59] = b'\n'; // Set the newline once at the end

        for i in (0..n_lines).rev() {
            // Fill only the letter positions, skipping spaces and the newline
            for word_idx in 0..10 {
                let start = word_idx * 6; // Each word + space block is 6 bytes
                for i in 0..5 {
                    line_buffer[start + i] = rng.random_range(b'a'..b'z' + 1);
                }
            }

            // Write the raw bytes directly to both files
            writer_from.write_all(&line_buffer)?;
            // make changes in the file
            if num_differences == 0 {
                writer_to.write_all(&line_buffer)?;
            } else {
                if i % change_every_n_lines == 0 && n_lines - i > 2 {
                    line_buffer[5] = CHANGE_INDICATION_CHAR;
                }
                writer_to.write_all(&line_buffer)?;
                line_buffer[5] = b' ';
            }
        }

        // create last line
        let missing = (bytes - n_lines * LINE_LENGTH as u64) as usize;
        if missing > 0 {
            for word_idx in 0..10 {
                let start = word_idx * 6; // Each word + space block is 6 bytes
                for i in 0..5 {
                    line_buffer[start + i] = rng.random_range(b'a'..b'z' + 1);
                }
            }
            line_buffer[missing - 1] = b'\n';
            writer_from.write_all(&line_buffer[0..missing])?;
            writer_to.write_all(&line_buffer[0..missing])?;
        }

        writer_from.flush()?;
        writer_to.flush()?;

        Ok(())
    }
}

/// Benchmark tools which are designed to call the compiled executable.
pub mod bench_binary {
    use std::process::Command;

    use crate::benchmark::str_to_args;

    pub fn bench_binary(program: &str, cmd_args: &str) -> std::process::ExitStatus {
        let args = str_to_args(cmd_args);
        Command::new(program)
            .args(args)
            .status()
            .expect("Failed to execute binary")
    }
}
