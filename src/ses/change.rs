// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::ses::edit::EditScript;

/// Edit operation on an individual element between two sequences.
#[derive(Debug, PartialEq, Eq)]
pub enum Change<'a, T> {
    /// Element present and equal in both sequences.
    Equal(&'a T, &'a T),
    /// Element removed from the original sequence.
    Delete(&'a T),
    /// Element added to the modified sequence.
    Insert(&'a T),
}

impl<'a, T> Clone for Change<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Copy for Change<'a, T> {}

/// Zero-copy lazy iterator yielding [`Change`] items without recursion.
pub struct ChangesIter<'s, 'a, T> {
    old: &'a [T],
    new: &'a [T],
    script: &'s EditScript,
    hunk_idx: usize,
    old_idx: usize,
    new_idx: usize,
    insert_idx: usize,
}

impl<'s, 'a, T> ChangesIter<'s, 'a, T> {
    pub(crate) fn new(script: &'s EditScript, old: &'a [T], new: &'a [T]) -> Self {
        Self {
            old,
            new,
            script,
            hunk_idx: 0,
            old_idx: 0,
            new_idx: 0,
            insert_idx: 0,
        }
    }
}

impl<'s, 'a, T> Iterator for ChangesIter<'s, 'a, T> {
    type Item = Change<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        let hunks = self.script.hunks();

        loop {
            if self.hunk_idx < hunks.len() {
                let hunk = &hunks[self.hunk_idx];
                let old_r = hunk.old_range();
                let new_r = hunk.new_range();

                if self.old_idx < old_r.start {
                    let old_item = &self.old[self.old_idx];
                    let new_item = &self.new[self.new_idx];
                    self.old_idx += 1;
                    self.new_idx += 1;
                    return Some(Change::Equal(old_item, new_item));
                }

                if self.old_idx < old_r.end {
                    let item = &self.old[self.old_idx];
                    self.old_idx += 1;
                    return Some(Change::Delete(item));
                }

                let ins_pos = new_r.start + self.insert_idx;
                if ins_pos < new_r.end {
                    let item = &self.new[ins_pos];
                    self.insert_idx += 1;
                    self.new_idx += 1;
                    return Some(Change::Insert(item));
                }

                self.hunk_idx += 1;
                self.insert_idx = 0;
                continue;
            }

            if self.old_idx < self.old.len() {
                let old_item = &self.old[self.old_idx];
                let new_item = &self.new[self.new_idx];
                self.old_idx += 1;
                self.new_idx += 1;
                return Some(Change::Equal(old_item, new_item));
            }

            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ses::diff_lines;

    #[test]
    fn test_iter_changes() {
        let a = ["a", "b"];
        let b = ["a", "c"];
        let diff = diff_lines(&a, &b);

        let changes: Vec<_> = diff.iter_changes().collect();
        assert_eq!(
            changes,
            vec![
                Change::Equal(&"a", &"a"),
                Change::Delete(&"b"),
                Change::Insert(&"c"),
            ]
        );
    }

    #[test]
    fn test_iter_changes_preserves_both_equal_items() {
        #[derive(Debug)]
        struct Normalized<'a> {
            raw: &'a [u8],
            key: &'a [u8],
        }
        impl PartialEq for Normalized<'_> {
            fn eq(&self, other: &Self) -> bool {
                self.key == other.key
            }
        }
        impl AsRef<[u8]> for Normalized<'_> {
            fn as_ref(&self) -> &[u8] {
                self.key
            }
        }

        let a = [
            Normalized {
                raw: b"\ta",
                key: b"    a",
            },
            Normalized {
                raw: b"b",
                key: b"b",
            },
        ];
        let b = [
            Normalized {
                raw: b"    a",
                key: b"    a",
            },
            Normalized {
                raw: b"b",
                key: b"b",
            },
        ];

        let diff = diff_lines(&a, &b);
        let changes: Vec<_> = diff.iter_changes().collect();
        assert_eq!(changes.len(), 2);
        match changes[0] {
            Change::Equal(left, right) => {
                assert_eq!(left.raw, b"\ta");
                assert_eq!(right.raw, b"    a");
            }
            other => panic!("expected Equal, got {other:?}"),
        }
    }
}
