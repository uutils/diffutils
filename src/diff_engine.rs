// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Internal sequence diff engine wrapping the `similar` crate.

use similar::{capture_diff_slices, Algorithm, DiffOp};

/// Represents an element difference between two sequences.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Change<T> {
    /// Present only in the expected (old / left) sequence.
    Left(T),
    /// Present only in the actual (new / right) sequence.
    Right(T),
    /// Present in both sequences.
    Both(T, T),
}

/// Computes the difference between two slices using Eugene Myers' linear-space algorithm.
///
/// Returns a vector of `Change<&'a T>` matching the sequence changes.
pub fn diff_slices<'a, T: std::hash::Hash + Eq>(
    expected: &'a [T],
    actual: &'a [T],
) -> Vec<Change<&'a T>> {
    let ops = capture_diff_slices(Algorithm::Myers, expected, actual);
    let mut changes = Vec::new();

    for op in ops {
        match op {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                for i in 0..len {
                    changes.push(Change::Both(
                        &expected[old_index + i],
                        &actual[new_index + i],
                    ));
                }
            }
            DiffOp::Delete {
                old_index, old_len, ..
            } => {
                for i in 0..old_len {
                    changes.push(Change::Left(&expected[old_index + i]));
                }
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for i in 0..new_len {
                    changes.push(Change::Right(&actual[new_index + i]));
                }
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                for i in 0..old_len {
                    changes.push(Change::Left(&expected[old_index + i]));
                }
                for i in 0..new_len {
                    changes.push(Change::Right(&actual[new_index + i]));
                }
            }
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_slices() {
        let a: &[&[u8]] = &[b"alpha", b"beta", b"gamma"];
        let changes = diff_slices(a, a);
        assert_eq!(
            changes,
            vec![
                Change::Both(&a[0], &a[0]),
                Change::Both(&a[1], &a[1]),
                Change::Both(&a[2], &a[2]),
            ]
        );
    }

    #[test]
    fn test_empty_slices() {
        let empty: &[&[u8]] = &[];
        let items: &[&[u8]] = &[b"a"];
        assert_eq!(diff_slices(empty, empty), vec![]);
        assert_eq!(diff_slices(empty, items), vec![Change::Right(&items[0])]);
        assert_eq!(diff_slices(items, empty), vec![Change::Left(&items[0])]);
    }

    #[test]
    fn test_modifications_and_replacements() {
        let a: &[&[u8]] = &[b"apple", b"banana", b"cherry"];
        let b: &[&[u8]] = &[b"apple", b"blueberry", b"cherry"];
        let changes = diff_slices(a, b);
        assert_eq!(
            changes,
            vec![
                Change::Both(&a[0], &b[0]),
                Change::Left(&a[1]),
                Change::Right(&b[1]),
                Change::Both(&a[2], &b[2]),
            ]
        );
    }

    #[test]
    fn test_insertions_and_deletions() {
        let a: &[&[u8]] = &[b"line1", b"line2", b"line3"];
        let b: &[&[u8]] = &[b"line1", b"line1.5", b"line3", b"line4"];
        let changes = diff_slices(a, b);
        assert_eq!(
            changes,
            vec![
                Change::Both(&a[0], &b[0]),
                Change::Left(&a[1]),
                Change::Right(&b[1]),
                Change::Both(&a[2], &b[2]),
                Change::Right(&b[3]),
            ]
        );
    }
}
