// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

//! Comparison of two directories, entry by entry, descending into
//! subdirectories with `-r`.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::ffi::{OsStr, OsString};
use std::fs::{self, Metadata};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::diff::{compare_files, Outcome};
use crate::params::{MissingFile, Params};
use crate::utils::{
    fold_file_name_case, quote_file_name, quote_shell_argument, report_failure_to_read_input_file,
    single_quote,
};

/// Compares `params.from` and `params.to` according to their types: the
/// entries of directories, the targets of symbolic links that are not
/// followed, or the contents of files. `option_args` are the command line
/// options, repeated in the header printed before the differences of each
/// pair of files found in directories.
pub fn compare(params: &Params, option_args: &[OsString]) -> Outcome {
    let mut walker = DirectoryWalker {
        params,
        option_args,
        ancestors: Vec::new(),
    };
    walker.compare_entries(&params.from, &params.to, Level::Top)
}

/// Whether files are the operands of diff, or found in directories.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Level {
    Top,
    Nested,
}

struct DirectoryWalker<'a> {
    params: &'a Params,
    option_args: &'a [OsString],
    /// Canonical paths of the directories being compared, from the top, to
    /// detect symbolic links looping back to one of them.
    ancestors: Vec<PathBuf>,
}

impl DirectoryWalker<'_> {
    fn compare_directories(&mut self, from_dir: &OsStr, to_dir: &OsStr, level: Level) -> Outcome {
        let mut outcome = Outcome::Identical;
        let mut canonical_dirs = Vec::new();
        for dir in [from_dir, to_dir] {
            let Ok(canonical) = fs::canonicalize(dir) else {
                // A directory missing on purpose (-N) has no entries.
                continue;
            };
            if self.ancestors.contains(&canonical) {
                eprintln!(
                    "{}: {}: recursive directory loop",
                    self.params.executable.to_string_lossy(),
                    dir.to_string_lossy()
                );
                return Outcome::Trouble;
            }
            canonical_dirs.push(canonical);
        }
        let (Some(mut from_names), Some(mut to_names)) = (self.list(from_dir), self.list(to_dir))
        else {
            return Outcome::Trouble;
        };
        if let (Level::Top, Some(start)) = (level, &self.params.starting_file) {
            let start = self.name_key(start).into_owned();
            from_names.retain(|name| *self.name_key(name) >= *start);
            to_names.retain(|name| *self.name_key(name) >= *start);
        }
        let ancestors_len = self.ancestors.len();
        self.ancestors.extend(canonical_dirs);

        let entries = pair_entries(&from_names, &to_names, |from, to| {
            self.name_key(from).cmp(&self.name_key(to))
        });
        for entry in entries {
            let entry_outcome = match entry {
                Entry::Both(from, to) => {
                    self.compare_entries(&join(from_dir, from), &join(to_dir, to), Level::Nested)
                }
                Entry::FromOnly(name) if self.params.missing_file == MissingFile::Empty => {
                    self.compare_entries(&join(from_dir, name), &join(to_dir, name), Level::Nested)
                }
                Entry::FromOnly(name) => report_only_in(from_dir, name),
                Entry::ToOnly(name) if self.params.missing_file == MissingFile::Error => {
                    report_only_in(to_dir, name)
                }
                Entry::ToOnly(name) => {
                    self.compare_entries(&join(from_dir, name), &join(to_dir, name), Level::Nested)
                }
            };
            outcome = outcome.max(entry_outcome);
        }
        self.ancestors.truncate(ancestors_len);
        outcome
    }

    /// Returns the sorted names of the entries of `dir`, or `None` after
    /// reporting an error. A directory missing on purpose (-N) is empty.
    fn list(&self, dir: &OsStr) -> Option<Vec<OsString>> {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && self.params.missing_file != MissingFile::Error =>
            {
                return Some(Vec::new());
            }
            Err(error) => {
                self.report_error(dir, &error);
                return None;
            }
        };
        let mut names = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) => names.push(entry.file_name()),
                Err(error) => {
                    self.report_error(dir, &error);
                    return None;
                }
            }
        }
        names.sort_unstable_by(|a, b| {
            self.name_key(a)
                .cmp(&self.name_key(b))
                .then_with(|| a.as_encoded_bytes().cmp(b.as_encoded_bytes()))
        });
        Some(names)
    }

    /// Returns what entry names are sorted and matched by: the names
    /// themselves, or their lowercase form with --ignore-file-name-case.
    fn name_key<'n>(&self, name: &'n OsStr) -> Cow<'n, [u8]> {
        if self.params.ignore_file_name_case {
            fold_file_name_case(name)
        } else {
            Cow::Borrowed(name.as_encoded_bytes())
        }
    }

    /// Compares two entries of the compared directories, one of which may be
    /// missing when missing files are treated as empty.
    fn compare_entries(&mut self, from: &OsStr, to: &OsStr, level: Level) -> Outcome {
        let (Ok(from_metadata), Ok(to_metadata)) = (self.metadata(from), self.metadata(to)) else {
            return Outcome::Trouble;
        };
        match (from_metadata, to_metadata) {
            (Some(from_metadata), Some(to_metadata))
                if from_metadata.is_dir() && to_metadata.is_dir() =>
            {
                if same_file::is_same_file(from, to).unwrap_or(false) {
                    Outcome::Identical
                } else if level == Level::Top || self.params.recursive {
                    self.compare_directories(from, to, level)
                } else {
                    println!(
                        "Common subdirectories: {} and {}",
                        from.to_string_lossy(),
                        to.to_string_lossy()
                    );
                    Outcome::Identical
                }
            }
            (Some(from_metadata), Some(to_metadata))
                if from_metadata.is_symlink() && to_metadata.is_symlink() =>
            {
                self.compare_symbolic_links(from, to)
            }
            (Some(from_metadata), Some(to_metadata))
                if !(from_metadata.is_file() && to_metadata.is_file()) =>
            {
                let (from_kind, to_kind) = (file_kind(&from_metadata), file_kind(&to_metadata));
                println!(
                    "File {} is a {from_kind} while file {} is a {to_kind}",
                    from.to_string_lossy(),
                    to.to_string_lossy()
                );
                Outcome::Different
            }
            // A directory on one side only, the other being missing on
            // purpose (-N), is compared with an empty directory.
            (Some(metadata), None) | (None, Some(metadata)) if metadata.is_dir() => {
                if self.params.recursive {
                    self.compare_directories(from, to, Level::Nested)
                } else {
                    println!(
                        "Common subdirectories: {} and {}",
                        from.to_string_lossy(),
                        to.to_string_lossy()
                    );
                    Outcome::Identical
                }
            }
            // The operands themselves are compared without a header, as when
            // they are not directories.
            _ if level == Level::Top => compare_files(self.params, None),
            _ => self.compare_files(from, to),
        }
    }

    /// Compares the targets of two symbolic links.
    fn compare_symbolic_links(&self, from: &OsStr, to: &OsStr) -> Outcome {
        let (from_target, to_target) = match (fs::read_link(from), fs::read_link(to)) {
            (Ok(from_target), Ok(to_target)) => (from_target, to_target),
            (Err(error), _) => {
                self.report_error(from, &error);
                return Outcome::Trouble;
            }
            (_, Err(error)) => {
                self.report_error(to, &error);
                return Outcome::Trouble;
            }
        };
        if from_target == to_target {
            if self.params.report_identical_files {
                println!(
                    "Files {} and {} are identical",
                    from.to_string_lossy(),
                    to.to_string_lossy()
                );
            }
            return Outcome::Identical;
        }
        let mut message = b"Symbolic links ".to_vec();
        for (link, target, separator) in [
            (from, from_target.as_os_str(), &b" and "[..]),
            (to, to_target.as_os_str(), b" differ\n"),
        ] {
            message.extend(single_quote(link.as_encoded_bytes()));
            message.extend_from_slice(b" -> ");
            message.extend(single_quote(target.as_encoded_bytes()));
            message.extend_from_slice(separator);
        }
        if io::stdout().write_all(&message).is_err() {
            return Outcome::Trouble;
        }
        Outcome::Different
    }

    /// Compares two files found in the compared directories.
    fn compare_files(&self, from: &OsStr, to: &OsStr) -> Outcome {
        let params = Params {
            from: from.to_os_string(),
            to: to.to_os_string(),
            ..self.params.clone()
        };
        let mut header = b"diff".to_vec();
        for arg in self.option_args {
            header.push(b' ');
            header.extend_from_slice(&quote_shell_argument(arg.as_encoded_bytes()));
        }
        for path in [from, to] {
            header.push(b' ');
            header.extend_from_slice(&quote_file_name(path.as_encoded_bytes()));
        }
        header.push(b'\n');
        compare_files(&params, Some(&header))
    }

    /// Returns the metadata of `path`, `None` if it is missing on purpose
    /// (-N), or an error after reporting it.
    fn metadata(&self, path: &OsStr) -> Result<Option<Metadata>, ()> {
        let metadata = if self.params.no_dereference {
            fs::symlink_metadata(path)
        } else {
            fs::metadata(path)
        };
        match metadata {
            Ok(metadata) => Ok(Some(metadata)),
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && self.params.missing_file != MissingFile::Error
                    && fs::symlink_metadata(path).is_err() =>
            {
                Ok(None)
            }
            Err(error) => {
                self.report_error(path, &error);
                Err(())
            }
        }
    }

    fn report_error(&self, path: &OsStr, error: &io::Error) {
        report_failure_to_read_input_file(&self.params.executable, &path.to_os_string(), error);
    }
}

/// An entry of either or both of the compared directories.
#[derive(Debug, PartialEq)]
enum Entry<'a> {
    Both(&'a OsStr, &'a OsStr),
    FromOnly(&'a OsStr),
    ToOnly(&'a OsStr),
}

/// Merges two lists of names sorted by `order`, pairing the names that are
/// equal according to it.
fn pair_entries<'a>(
    from_names: &'a [OsString],
    to_names: &'a [OsString],
    order: impl Fn(&OsStr, &OsStr) -> Ordering,
) -> Vec<Entry<'a>> {
    let mut entries = Vec::with_capacity(from_names.len().max(to_names.len()));
    let mut from_names = from_names.iter().map(OsString::as_os_str).peekable();
    let mut to_names = to_names.iter().map(OsString::as_os_str).peekable();
    loop {
        let entry = match (from_names.peek(), to_names.peek()) {
            (None, None) => return entries,
            (Some(&from), Some(&to)) => match order(from, to) {
                Ordering::Equal => Entry::Both(from, to),
                Ordering::Less => Entry::FromOnly(from),
                Ordering::Greater => Entry::ToOnly(to),
            },
            (Some(&from), None) => Entry::FromOnly(from),
            (None, Some(&to)) => Entry::ToOnly(to),
        };
        match entry {
            Entry::Both(..) => {
                from_names.next();
                to_names.next();
            }
            Entry::FromOnly(_) => {
                from_names.next();
            }
            Entry::ToOnly(_) => {
                to_names.next();
            }
        }
        entries.push(entry);
    }
}

fn report_only_in(dir: &OsStr, name: &OsStr) -> Outcome {
    println!(
        "Only in {}: {}",
        dir.to_string_lossy(),
        name.to_string_lossy()
    );
    Outcome::Different
}

/// Joins a directory and an entry name, without doubling the separator when
/// the directory was given with trailing slashes.
fn join(dir: &OsStr, name: &OsStr) -> OsString {
    let dir: PathBuf = Path::new(dir).components().collect();
    dir.join(name).into_os_string()
}

/// Describes the type of a file, as in "File X is a fifo while ...".
fn file_kind(metadata: &Metadata) -> &'static str {
    let file_type = metadata.file_type();
    // Only seen with --no-dereference, as symbolic links are followed otherwise.
    if file_type.is_symlink() {
        return "symbolic link";
    }
    if file_type.is_dir() {
        return "directory";
    }
    if file_type.is_file() {
        return if metadata.len() == 0 {
            "regular empty file"
        } else {
            "regular file"
        };
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        if file_type.is_fifo() {
            return "fifo";
        }
        if file_type.is_socket() {
            return "socket";
        }
        if file_type.is_char_device() {
            return "character special file";
        }
        if file_type.is_block_device() {
            return "block special file";
        }
    }
    "weird file"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_paired_by_name() {
        let names =
            |names: &[&str]| -> Vec<OsString> { names.iter().map(OsString::from).collect() };
        let (from, to) = (names(&["b", "d", "e", "x"]), names(&["a", "d", "x", "z"]));
        let entries = pair_entries(&from, &to, |a, b| a.cmp(b));
        let name = OsStr::new;
        assert_eq!(
            entries,
            [
                Entry::ToOnly(name("a")),
                Entry::FromOnly(name("b")),
                Entry::Both(name("d"), name("d")),
                Entry::FromOnly(name("e")),
                Entry::Both(name("x"), name("x")),
                Entry::ToOnly(name("z")),
            ]
        );
        assert_eq!(pair_entries(&[], &[], |a, b| a.cmp(b)), []);
    }
}
