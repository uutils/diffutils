// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::ses::{
    edit::{EditScript, Hunk},
    optimize::bitset::Bitset,
};

/// Adjusts insertion and deletion boundaries across identical elements
/// to merge adjacent changes and prefer cleaner, intuitive diff boundaries.
///
/// When identical elements appear at the edges of an edit (such as repeated
/// lines, blank lines, or closing delimiters), an edit can mathematically be
/// placed at multiple valid positions with identical minimal edit distance.
/// Boundary shifting slides the changes to join separate hunks whenever
/// possible, favoring shifts toward earlier matching positions to coalesce
/// changes, and moving forward to place changes at natural boundaries.
pub(crate) fn shift_boundaries<T, F>(a: &[T], b: &[T], hunks: &[Hunk], mut equals: F) -> EditScript
where
    F: FnMut(&T, &T) -> bool,
{
    let n = a.len();
    let m = b.len();

    // If there are no hunks or either sequence is empty, no shifts can occur.
    if hunks.is_empty() {
        return EditScript::default();
    }

    if n == 0 || m == 0 {
        return EditScript::new(hunks.to_vec());
    }

    // Boolean masks tracking modified elements in each sequence.
    let mut changed_a = Bitset::new(n);
    let mut changed_b = Bitset::new(m);

    // Mark all changed ranges directly using slice operations.
    for hunk in hunks {
        changed_a.set_range(hunk.old_range());
        changed_b.set_range(hunk.new_range());
    }

    // Shift deletions (sequence A) against insertions (sequence B),
    // then shift insertions (sequence B) against deletions (sequence A).
    shift_sequence(a, &mut changed_a, &changed_b, &mut equals);
    shift_sequence(b, &mut changed_b, &changed_a, &mut equals);

    // Reconstruct coalesced hunks from the adjusted masks.
    reconstruct_hunks(&changed_a, &changed_b, hunks.len())
}

/// Shifts change boundaries for a single sequence against the other sequence's changes.
fn shift_sequence<T, F>(elements: &[T], changed: &mut Bitset, other_changed: &Bitset, mut equals: F)
where
    F: FnMut(&T, &T) -> bool,
{
    let len = elements.len();
    let other_len = other_changed.len();
    let mut i = 0;
    let mut j = 0;

    while i < len {
        // Advance i and j together through unchanged elements.
        while i < len && !changed.get(i) {
            advance_other_unchanged(other_changed, &mut j);
            i += 1;
        }

        if i >= len {
            break;
        }

        let mut start = i;

        // Find the end of this run of changes.
        i += 1;
        while i < len && changed.get(i) {
            i += 1;
        }
        while j < other_len && other_changed.get(j) {
            j += 1;
        }

        let mut corresponding;

        loop {
            let runlength = i - start;

            // Backward shift: while previous unchanged element matches the last changed element.
            // This merges with preceding changed regions whenever possible.
            while start > 0 && equals(&elements[start - 1], &elements[i - 1]) {
                start -= 1;
                i -= 1;
                changed.set(start);
                changed.clear(i);

                while start > 0 && changed.get(start - 1) {
                    start -= 1;
                }
                step_back_other_unchanged(other_changed, &mut j);
            }

            // Record whether this changed run corresponds to a change in the other sequence.
            corresponding = if is_corresponding(other_changed, j) {
                i
            } else {
                len
            };

            // Forward shift: while first changed element matches following unchanged element.
            // This merges with subsequent changed regions.
            while i < len && equals(&elements[start], &elements[i]) {
                changed.clear(start);
                changed.set(i);
                start += 1;
                i += 1;

                while i < len && changed.get(i) {
                    i += 1;
                }

                step_forward_other_unchanged(other_changed, &mut j, i, &mut corresponding);
            }

            // Stop when shifting ceases to change the run length (no new merges).
            if runlength == i - start {
                break;
            }
        }

        // Move back to the corresponding run in the other sequence if shifted too far.
        while corresponding < i {
            start -= 1;
            i -= 1;
            changed.set(start);
            changed.clear(i);

            step_back_other_unchanged(other_changed, &mut j);
        }
    }
}

/// Advances `j` in the other sequence past any changes and one matching unchanged element.
#[inline]
fn advance_other_unchanged(other: &Bitset, j: &mut usize) {
    while *j < other.len() && other.get(*j) {
        *j += 1;
    }
    if *j < other.len() {
        *j += 1;
    }
}

/// Steps `j` backward in the other sequence past one matching unchanged element
/// and any preceding changes.
#[inline]
fn step_back_other_unchanged(other: &Bitset, j: &mut usize) {
    if *j > 0 {
        *j -= 1;
        while *j > 0 && other.get(*j) {
            *j -= 1;
        }
    }
}

/// Steps `j` forward in the other sequence past one matching unchanged element
/// and following changes, updating the correspondence index.
#[inline]
fn step_forward_other_unchanged(
    other: &Bitset,
    j: &mut usize,
    i: usize,
    corresponding: &mut usize,
) {
    *j += 1;
    while *j < other.len() && other.get(*j) {
        *corresponding = i;
        *j += 1;
    }
}

/// Returns true if the element immediately preceding `j` in the other sequence is changed.
#[inline]
fn is_corresponding(other: &Bitset, j: usize) -> bool {
    j > 0 && j - 1 < other.len() && other.get(j - 1)
}

/// Reconstructs coalesced hunks by scanning backward from the ends of both sequences.
fn reconstruct_hunks(changed_a: &Bitset, changed_b: &Bitset, capacity_hint: usize) -> EditScript {
    let mut hunks = Vec::with_capacity(capacity_hint);
    let mut i = changed_a.len();
    let mut j = changed_b.len();

    while i > 0 || j > 0 {
        if (i > 0 && changed_a.get(i - 1)) || (j > 0 && changed_b.get(j - 1)) {
            let old_end = i;
            while i > 0 && changed_a.get(i - 1) {
                i -= 1;
            }
            let old_start = i;

            let new_end = j;
            while j > 0 && changed_b.get(j - 1) {
                j -= 1;
            }
            let new_start = j;

            hunks.push(Hunk::new(old_start..old_end, new_start..new_end));
        } else {
            i = i.saturating_sub(1);
            j = j.saturating_sub(1);
        }
    }

    hunks.reverse();
    EditScript::new(hunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shift_identical_repeat() {
        let a = ["x", "a", "a", "a", "y"];
        let b = ["x", "a", "a", "y"];
        let initial_hunks = vec![Hunk::new(1..2, 1..1)];
        let shifted = shift_boundaries(&a, &b, &initial_hunks, |x, y| x == y);
        assert_eq!(shifted.hunks().len(), 1);
        assert_eq!(shifted.hunks()[0].old_range(), 3..4);
    }
}
