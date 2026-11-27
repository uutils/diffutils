// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::ses::edit::EditScript;
#[cfg(test)]
use crate::ses::edit::Hunk;

/// Computed shortest edit script paired with borrowed input sequences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff<'a, T> {
    pub(crate) script: EditScript,
    pub(crate) old: &'a [T],
    pub(crate) new: &'a [T],
}

impl<'a, T> Diff<'a, T> {
    #[inline]
    pub(crate) fn from_parts(script: EditScript, old: &'a [T], new: &'a [T]) -> Self {
        debug_assert!(
            Self::is_valid(&script, old.len(), new.len()),
            "invalid edit script: hunks do not match slice dimensions or are non-monotonic"
        );
        Self { script, old, new }
    }

    pub(crate) fn is_valid(script: &EditScript, old_len: usize, new_len: usize) -> bool {
        let hunks = script.hunks();

        if hunks.is_empty() {
            return old_len == new_len;
        }

        let mut prev_old_end = 0;
        let mut prev_new_end = 0;

        for hunk in hunks {
            let old_r = hunk.old_range();
            let new_r = hunk.new_range();

            if old_r.start > old_r.end || old_r.end > old_len {
                return false;
            }
            if new_r.start > new_r.end || new_r.end > new_len {
                return false;
            }
            if old_r.is_empty() && new_r.is_empty() {
                return false;
            }
            if old_r.start < prev_old_end || new_r.start < prev_new_end {
                return false;
            }
            if (old_r.start - prev_old_end) != (new_r.start - prev_new_end) {
                return false;
            }

            prev_old_end = old_r.end;
            prev_new_end = new_r.end;
        }

        (old_len - prev_old_end) == (new_len - prev_new_end)
    }

    #[cfg(test)]
    pub(crate) fn hunks(&self) -> &[Hunk] {
        self.script.hunks()
    }

    /// Lazily iterates over individual item changes (`Equal`, `Delete`, `Insert`).
    pub fn iter_changes(&self) -> crate::ses::ChangesIter<'_, 'a, T> {
        crate::ses::ChangesIter::new(&self.script, self.old, self.new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_validation_valid_cases() {
        assert!(Diff::<i32>::is_valid(&EditScript::default(), 0, 0));
        assert!(Diff::<i32>::is_valid(&EditScript::default(), 5, 5));

        let ins = Hunk::new(1..1, 1..2);
        assert!(Diff::<i32>::is_valid(&EditScript::new(vec![ins]), 2, 3));

        let del = Hunk::new(0..1, 0..0);
        assert!(Diff::<i32>::is_valid(&EditScript::new(vec![del]), 2, 1));

        let h1 = Hunk::new(1..2, 1..1);
        let h2 = Hunk::new(3..3, 2..3);
        assert!(Diff::<i32>::is_valid(&EditScript::new(vec![h1, h2]), 4, 4));
    }

    #[test]
    fn test_diff_validation_invalid_cases() {
        assert!(!Diff::<i32>::is_valid(&EditScript::default(), 5, 2));

        let out_of_bounds = Hunk::new(0..5, 0..1);
        assert!(!Diff::<i32>::is_valid(
            &EditScript::new(vec![out_of_bounds]),
            3,
            3
        ));

        let h1 = Hunk::new(0..2, 0..1);
        let h2 = Hunk::new(1..3, 1..2);
        assert!(!Diff::<i32>::is_valid(&EditScript::new(vec![h1, h2]), 4, 4));

        let h1 = Hunk::new(0..1, 0..1);
        let h2 = Hunk::new(3..4, 2..3);
        assert!(!Diff::<i32>::is_valid(&EditScript::new(vec![h1, h2]), 4, 4));

        let h = Hunk::new(0..1, 0..1);
        assert!(!Diff::<i32>::is_valid(&EditScript::new(vec![h]), 3, 2));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "invalid edit script")]
    fn test_diff_from_parts_panics_on_mismatched_empty_script() {
        let old = [1, 2, 3];
        let new = [1];
        let _ = Diff::from_parts(EditScript::default(), &old, &new);
    }
}
