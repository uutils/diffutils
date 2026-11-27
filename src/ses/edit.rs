// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use std::ops::Range;

/// A single edit hunk (`old_range` removed from A, `new_range` inserted from B).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hunk {
    old_range: Range<usize>,
    new_range: Range<usize>,
}

impl Hunk {
    pub(crate) fn new(old_range: Range<usize>, new_range: Range<usize>) -> Self {
        Self {
            old_range,
            new_range,
        }
    }

    #[inline]
    pub(crate) fn old_range(&self) -> Range<usize> {
        self.old_range.clone()
    }

    #[inline]
    pub(crate) fn new_range(&self) -> Range<usize> {
        self.new_range.clone()
    }
}

/// Non-overlapping, monotonic sequence of edit [`Hunk`]s.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct EditScript {
    hunks: Vec<Hunk>,
}

impl EditScript {
    pub(crate) fn new(hunks: Vec<Hunk>) -> Self {
        Self { hunks }
    }

    pub(crate) fn hunks(&self) -> &[Hunk] {
        &self.hunks
    }

    pub(crate) fn into_hunks(self) -> Vec<Hunk> {
        self.hunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_edit_script() {
        let script = EditScript::default();
        assert!(script.hunks().is_empty());
    }

    #[test]
    fn test_hunk_ranges() {
        let hunk = Hunk::new(0..2, 0..1);
        assert_eq!(hunk.old_range(), 0..2);
        assert_eq!(hunk.new_range(), 0..1);

        let script = EditScript::new(vec![hunk.clone()]);
        assert_eq!(script.hunks(), &[hunk.clone()]);
        assert_eq!(script.into_hunks(), vec![hunk]);
    }
}
