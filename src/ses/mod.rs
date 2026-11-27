// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

//! Zero-dependency, linear-space Myers diff engine (`O((N + M) D)` time, `O(N + M)` space).
//!
//! Computes the Shortest Edit Script between two sequences using:
//! - Common prefix/suffix pruning (`O(N)` fast-path)
//! - Token interning into `u32` IDs via an internal `FxHashMap`
//! - Bidirectional middle-snake divide-and-conquer with a single reusable diagonal `Workspace`
//! - Semantic boundary shifting matching GNU `diff` hunk alignment

pub(crate) mod algorithm;
mod change;
mod diff;
mod edit;
pub(crate) mod optimize;

pub use change::{Change, ChangesIter};
pub use diff::Diff;

use edit::{EditScript, Hunk};

/// Computes a line-by-line diff across pre-split slices using collision-free token interning.
///
/// # Examples
///
/// ```rust
/// use diffutilslib::ses::{diff_lines, Change};
///
/// let old = ["line 1", "line 2", "line 3"];
/// let new = ["line 1", "line 2 modified", "line 3"];
/// let diff = diff_lines(&old, &new);
///
/// let changes: Vec<_> = diff.iter_changes().collect();
/// assert_eq!(changes[1], Change::Delete(&"line 2"));
/// assert_eq!(changes[2], Change::Insert(&"line 2 modified"));
/// ```
pub fn diff_lines<'a, T>(old: &'a [T], new: &'a [T]) -> Diff<'a, T>
where
    T: AsRef<[u8]> + PartialEq,
{
    let trimmed = optimize::trim::trim_slices(old, new, |x, y| x == y);

    if trimmed.slice_a.is_empty() && trimmed.slice_b.is_empty() {
        return Diff::from_parts(EditScript::default(), old, new);
    }

    let mut interner = optimize::intern::TokenInterner::with_capacity(
        trimmed.slice_a.len() + trimmed.slice_b.len(),
    );
    let old_tokens = interner.intern(trimmed.slice_a);
    let new_tokens = interner.intern(trimmed.slice_b);

    let script = algorithm::myers::myers(old_tokens.len(), new_tokens.len(), |i, j| {
        old_tokens[i] == new_tokens[j]
    });

    let shifted =
        algorithm::shift::shift_boundaries(&old_tokens, &new_tokens, script.hunks(), |x, y| x == y);

    let mut final_hunks = Vec::with_capacity(shifted.hunks().len());
    for hunk in shifted.into_hunks() {
        let old_r = hunk.old_range();
        let new_r = hunk.new_range();
        final_hunks.push(Hunk::new(
            old_r.start + trimmed.prefix_len..old_r.end + trimmed.prefix_len,
            new_r.start + trimmed.prefix_len..new_r.end + trimmed.prefix_len,
        ));
    }

    Diff::from_parts(EditScript::new(final_hunks), old, new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_lines() {
        let old = ["line 1", "line 2", "line 3"];
        let new = ["line 1", "line 2 modified", "line 3"];
        let diff = diff_lines(&old, &new);

        assert_eq!(diff.hunks().len(), 1);
        assert_eq!(diff.hunks()[0].old_range(), 1..2);
        assert_eq!(diff.hunks()[0].new_range(), 1..2);
    }

    #[test]
    fn test_diff_byte_lines() {
        let old: [&[u8]; 3] = [b"line 1\n", b"line 2\n", b"line 3\n"];
        let new: [&[u8]; 3] = [b"line 1\n", b"line 2 modified\n", b"line 3\n"];
        let diff = diff_lines(&old, &new);

        assert_eq!(diff.hunks().len(), 1);
        assert_eq!(diff.hunks()[0].old_range(), 1..2);
        assert_eq!(diff.hunks()[0].new_range(), 1..2);
    }

    #[test]
    fn test_diff_lines_boundary_shifting() {
        let old = ["x", "a", "a", "a", "y"];
        let new = ["x", "a", "a", "y"];
        let diff = diff_lines(&old, &new);

        assert_eq!(diff.hunks().len(), 1);
        assert_eq!(diff.hunks()[0].old_range(), 3..4);
    }

    #[test]
    fn test_gnu_compat_edge_cases() {
        type ExpectedHunk = (std::ops::Range<usize>, std::ops::Range<usize>);
        type EdgeCase<'a> = (&'a [&'a str], &'a [&'a str], &'a [ExpectedHunk]);

        let cases: &[EdgeCase<'_>] = &[
            (
                &["fn foo() {", "    body();", "}", ""],
                &[
                    "fn foo() {",
                    "    body();",
                    "}",
                    "",
                    "fn bar() {",
                    "    body();",
                    "}",
                    "",
                ],
                &[(4..4, 4..8)],
            ),
            (
                &["header", "item", "item", "footer"],
                &["header", "item", "footer"],
                &[(2..3, 2..2)],
            ),
            (
                &["if cond {", "    a();", "}", "else {", "    b();", "}"],
                &["if cond {", "    a();", "}"],
                &[(3..6, 3..3)],
            ),
            (
                &["x", "a", "c", "d", "d", "a", "d", "c", "d", "c", "y"],
                &["x", "b", "b", "b", "a", "a", "d", "a", "y"],
                &[(1..1, 1..4), (2..5, 5..5), (7..10, 7..8)],
            ),
            (
                &["a", "b", "a", "b", "a", "b"],
                &["b", "a", "b", "a"],
                &[(0..1, 0..0), (5..6, 4..4)],
            ),
            (
                &["a", "b", "c", "d", "e"],
                &["e", "d", "c", "b", "a"],
                &[(0..4, 0..0), (5..5, 1..5)],
            ),
            (
                &["d", "c", "d"],
                &["c", "d", "b", "a", "e", "d"],
                &[(0..1, 0..0), (2..2, 1..5)],
            ),
        ];

        for &(a, b, expected) in cases {
            let diff = diff_lines(a, b);
            let actual: Vec<_> = diff
                .hunks()
                .iter()
                .map(|h| (h.old_range(), h.new_range()))
                .collect();
            assert_eq!(actual, expected, "mismatch for a={a:?}, b={b:?}");
        }
    }
}
