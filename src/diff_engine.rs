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
    let mut changes = Vec::with_capacity(expected.len() + actual.len());

    for op in ops {
        match op {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                changes.extend(
                    expected[old_index..old_index + len]
                        .iter()
                        .zip(&actual[new_index..new_index + len])
                        .map(|(e, a)| Change::Both(e, a)),
                );
            }
            DiffOp::Delete {
                old_index, old_len, ..
            } => {
                changes.extend(
                    expected[old_index..old_index + old_len]
                        .iter()
                        .map(Change::Left),
                );
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                changes.extend(
                    actual[new_index..new_index + new_len]
                        .iter()
                        .map(Change::Right),
                );
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                changes.extend(
                    expected[old_index..old_index + old_len]
                        .iter()
                        .map(Change::Left),
                );
                changes.extend(
                    actual[new_index..new_index + new_len]
                        .iter()
                        .map(Change::Right),
                );
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

    #[test]
    fn test_completely_disjoint_slices() {
        let a: &[&[u8]] = &[b"one", b"two", b"three"];
        let b: &[&[u8]] = &[b"four", b"five", b"six"];
        let changes = diff_slices(a, b);
        assert_eq!(
            changes,
            vec![
                Change::Left(&a[0]),
                Change::Left(&a[1]),
                Change::Left(&a[2]),
                Change::Right(&b[0]),
                Change::Right(&b[1]),
                Change::Right(&b[2]),
            ]
        );
    }

    #[test]
    fn test_asymmetric_replace() {
        // 1 element replaced by 3
        let a: &[&[u8]] = &[b"start", b"old", b"end"];
        let b: &[&[u8]] = &[b"start", b"new1", b"new2", b"new3", b"end"];
        let changes = diff_slices(a, b);
        assert_eq!(
            changes,
            vec![
                Change::Both(&a[0], &b[0]),
                Change::Left(&a[1]),
                Change::Right(&b[1]),
                Change::Right(&b[2]),
                Change::Right(&b[3]),
                Change::Both(&a[2], &b[4]),
            ]
        );

        // 3 elements replaced by 1
        let changes_rev = diff_slices(b, a);
        assert_eq!(
            changes_rev,
            vec![
                Change::Both(&b[0], &a[0]),
                Change::Left(&b[1]),
                Change::Left(&b[2]),
                Change::Left(&b[3]),
                Change::Right(&a[1]),
                Change::Both(&b[4], &a[2]),
            ]
        );
    }

    #[test]
    fn test_repeated_and_duplicate_elements() {
        let a: &[&[u8]] = &[b"dup", b"dup", b"dup"];
        let b: &[&[u8]] = &[b"dup", b"dup"];
        let changes = diff_slices(a, b);
        assert_eq!(
            changes,
            vec![
                Change::Both(&a[0], &b[0]),
                Change::Both(&a[1], &b[1]),
                Change::Left(&a[2]),
            ]
        );
    }

    #[test]
    fn test_empty_lines_and_binary_content() {
        let a: &[&[u8]] = &[b"", b"\x00\xFF\xFE", b""];
        let b: &[&[u8]] = &[b"", b"\x00\xFF\x01", b""];
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
    fn test_large_sparse_sequence() {
        let a: Vec<Vec<u8>> = (0..5000)
            .map(|i| format!("line {i}").into_bytes())
            .collect();
        let mut b = a.clone();
        b[2500] = b"modified line 2500".to_vec();

        let a_refs: Vec<&[u8]> = a.iter().map(Vec::as_slice).collect();
        let b_refs: Vec<&[u8]> = b.iter().map(Vec::as_slice).collect();

        let changes = diff_slices(&a_refs, &b_refs);
        assert_eq!(changes.len(), 5001); // 4999 both + 1 left + 1 right
        assert_eq!(changes[2500], Change::Left(&a_refs[2500]));
        assert_eq!(changes[2501], Change::Right(&b_refs[2500]));
    }

    #[test]
    fn test_myers_vs_lcs_difference() {
        // When multiple minimal edit paths exist, Myers' algorithm preserves common
        // runs and shifts the unique element, whereas traditional LCS kept 'b' stationary:
        //
        // LCS edit path:     delete 'a' at 4, insert 'a' at 5 (keeping 'b' matched at 5)
        // Myers edit path:   insert 'b' at 4, delete 'b' at 5 (keeping all four 'a's matched)
        let a: &[&[u8]] = &[b"a", b"a", b"a", b"a", b"b"];
        let b: &[&[u8]] = &[b"a", b"a", b"a", b"b", b"a"];
        let changes = diff_slices(a, b);
        assert_eq!(
            changes,
            vec![
                Change::Both(&a[0], &b[0]),
                Change::Both(&a[1], &b[1]),
                Change::Both(&a[2], &b[2]),
                Change::Right(&b[3]),
                Change::Both(&a[3], &b[4]),
                Change::Left(&a[4]),
            ]
        );
    }
}
