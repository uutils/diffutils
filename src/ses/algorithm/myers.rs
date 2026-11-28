// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::ses::edit::{EditScript, Hunk};
use std::ops::{Index, IndexMut, Range};

/// A storage buffer for diagonal endpoints, allowing natural mathematical
/// indexing with signed diagonal indices `k` in `[-max, max]`.
pub(crate) struct DiagonalBuffer {
    offset: isize,
    buffer: Vec<usize>,
}

impl DiagonalBuffer {
    /// Constructs a new `DiagonalBuffer` sized to store diagonals from `-max` to `+max` inclusive.
    pub fn new(max: usize) -> Self {
        Self {
            offset: max as isize,
            buffer: vec![0; 2 * max + 1],
        }
    }
}

impl Index<isize> for DiagonalBuffer {
    type Output = usize;

    #[inline]
    fn index(&self, k: isize) -> &Self::Output {
        &self.buffer[(k + self.offset) as usize]
    }
}

impl IndexMut<isize> for DiagonalBuffer {
    #[inline]
    fn index_mut(&mut self, k: isize) -> &mut Self::Output {
        &mut self.buffer[(k + self.offset) as usize]
    }
}

/// Preallocated workspace holding forward (`vf`) and backward (`vb`) diagonal buffers,
/// allocated once at the start of the algorithm to avoid allocations in the diffing loop.
pub(crate) struct Workspace {
    pub vf: DiagonalBuffer,
    pub vb: DiagonalBuffer,
}

impl Workspace {
    /// Allocates forward and backward diagonal buffers for sequences of combined length `max`.
    pub fn new(max: usize) -> Self {
        Self {
            vf: DiagonalBuffer::new(max),
            vb: DiagonalBuffer::new(max),
        }
    }
}

/// A maximal diagonal run of identical elements (`old[start_x..start_x + len] == new[start_y..start_y + len]`).
///
/// In Myers' divide-and-conquer bisection, the middle snake connects
/// a forward furthest-reaching path with a backward furthest-reaching path.
/// Coordinates are expressed relative to the containing edit box.
///
/// # Note on Myers' Paper Terminology
/// In Section 2 of Myers' 1986 paper (*"An O(ND) Difference Algorithm and Its Variations"*),
/// a "snake" is formally defined as an atomic search-frontier step: one edit move
/// (horizontal deletion or vertical insertion of cost 1) followed by zero or more diagonal edges
/// (matches of cost 0).
///
/// In Section 4 (the linear-space divide-and-conquer refinement) and in this struct,
/// `Snake` represents strictly the maximal diagonal run of identical elements (`Δx == Δy == len`)
/// where the forward and backward search frontiers meet. The edit moves are not part of
/// the snake itself; they belong to the subproblems on either side of it. This ensures
/// `Δx == Δy == len` by construction and allows clean bisection of [`EditBox`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Snake {
    pub start_x: usize,
    pub start_y: usize,
    pub len: usize,
}

impl Snake {
    /// Creates a new `Snake` starting at relative coordinates `(start_x, start_y)` of length `len`.
    #[inline]
    pub fn new(start_x: usize, start_y: usize, len: usize) -> Self {
        Self {
            start_x,
            start_y,
            len,
        }
    }

    /// End X-coordinate relative to the containing edit box (`start_x + len`).
    #[inline]
    pub fn end_x(&self) -> usize {
        self.start_x + self.len
    }

    /// End Y-coordinate relative to the containing edit box (`start_y + len`).
    #[inline]
    pub fn end_y(&self) -> usize {
        self.start_y + self.len
    }

    /// Returns `true` if this snake has zero length (a point match transition).
    #[cfg(test)]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A 2D bounding box in the edit graph representing a diff subproblem.
///
/// In Myers' 1986 paper (Section 4), sequence A runs along the horizontal X-axis
/// and sequence B runs along the vertical Y-axis. An `EditBox` bounds a 2D rectangle
/// `[old_range × new_range]` comparing `A[old_range]` against `B[new_range]`.
///
/// It requires two independent ranges because insertions and deletions advance each
/// sequence at different rates, meaning each subproblem can have different start/end
/// offsets and lengths.
///
/// Divide-and-conquer iteratively bisects this box around middle snakes into independent
/// left (top-left) and right (bottom-right) sub-boxes until reaching base cases (identical
/// matches or pure insertions/deletions) that emit as diff hunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditBox {
    /// Half-open range of indices in sequence A (horizontal X-axis).
    pub old_range: Range<usize>,
    /// Half-open range of indices in sequence B (vertical Y-axis).
    pub new_range: Range<usize>,
}

impl EditBox {
    /// Constructs a new `EditBox` from old and new index ranges.
    #[inline]
    pub fn new(old_range: Range<usize>, new_range: Range<usize>) -> Self {
        Self {
            old_range,
            new_range,
        }
    }

    /// Number of elements remaining in sequence A within this box.
    #[inline]
    pub fn old_len(&self) -> usize {
        self.old_range.len()
    }

    /// Number of elements remaining in sequence B within this box.
    #[inline]
    pub fn new_len(&self) -> usize {
        self.new_range.len()
    }

    /// Returns `true` if both old and new ranges are completely empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.old_range.is_empty() && self.new_range.is_empty()
    }

    /// Trims matching elements from the start of both ranges in-place,
    /// advancing `old_range.start` and `new_range.start`.
    pub fn trim_common_prefix<F>(&mut self, mut equals: F) -> usize
    where
        F: FnMut(usize, usize) -> bool,
    {
        let mut count = 0;
        while self.old_range.start < self.old_range.end
            && self.new_range.start < self.new_range.end
            && equals(self.old_range.start, self.new_range.start)
        {
            self.old_range.start += 1;
            self.new_range.start += 1;
            count += 1;
        }
        count
    }

    /// Trims matching elements from the end of both ranges in-place,
    /// decrementing `old_range.end` and `new_range.end`.
    pub fn trim_common_suffix<F>(&mut self, mut equals: F) -> usize
    where
        F: FnMut(usize, usize) -> bool,
    {
        let mut count = 0;
        while self.old_range.start < self.old_range.end
            && self.new_range.start < self.new_range.end
            && equals(self.old_range.end - 1, self.new_range.end - 1)
        {
            self.old_range.end -= 1;
            self.new_range.end -= 1;
            count += 1;
        }
        count
    }

    /// Bisects this edit box around a middle snake, producing independent `(left, right)` boxes.
    ///
    /// The snake itself represents identical tokens and is excluded from both boxes:
    /// - `left` covers `(old_start..old_start + snake.start_x, new_start..new_start + snake.start_y)`
    /// - `right` covers `(old_start + snake.end_x()..old_end, new_start + snake.end_y()..new_end)`
    #[inline]
    pub fn split_around(&self, snake: Snake) -> (EditBox, EditBox) {
        let left = EditBox::new(
            self.old_range.start..(self.old_range.start + snake.start_x),
            self.new_range.start..(self.new_range.start + snake.start_y),
        );
        let right = EditBox::new(
            (self.old_range.start + snake.end_x())..self.old_range.end,
            (self.new_range.start + snake.end_y())..self.new_range.end,
        );
        (left, right)
    }

    /// Converts an edit box where at least one side is empty into an edit `Hunk`.
    #[inline]
    pub fn into_hunk(self) -> Hunk {
        Hunk::new(self.old_range, self.new_range)
    }
}

/// Greedily advances `(x, y)` along a forward diagonal while elements match.
#[inline]
pub(crate) fn extend_forward<F>(
    mut x: usize,
    mut y: usize,
    max_x: usize,
    max_y: usize,
    x_off: usize,
    y_off: usize,
    equals: &mut F,
) -> (usize, usize)
where
    F: FnMut(usize, usize) -> bool,
{
    while x < max_x && y < max_y && equals(x_off + x, y_off + y) {
        x += 1;
        y += 1;
    }
    (x, y)
}

/// Greedily retreats `(x, y)` along a backward diagonal while elements match.
#[inline]
pub(crate) fn extend_backward<F>(
    mut x: usize,
    mut y: usize,
    x_off: usize,
    y_off: usize,
    equals: &mut F,
) -> (usize, usize)
where
    F: FnMut(usize, usize) -> bool,
{
    while x > 0 && y > 0 && equals(x_off + x - 1, y_off + y - 1) {
        x -= 1;
        y -= 1;
    }
    (x, y)
}

/// Appends a `Hunk` to the list, coalescing it with the preceding hunk if they are adjacent or overlapping.
#[inline]
pub(crate) fn push_or_coalesce(hunks: &mut Vec<Hunk>, hunk: Hunk) {
    if let Some(last) = hunks.last_mut() {
        let last_old = last.old_range();
        let last_new = last.new_range();
        let hunk_old = hunk.old_range();
        let hunk_new = hunk.new_range();

        if last_old.end >= hunk_old.start && last_new.end >= hunk_new.start {
            *last = Hunk::new(
                last_old.start..last_old.end.max(hunk_old.end),
                last_new.start..last_new.end.max(hunk_new.end),
            );
            return;
        }
    }
    hunks.push(hunk);
}

/// Computes the Shortest Edit Script (SES) in `O((N + M) * D)` time and `O(N + M)` space,
/// based on Eugene Myers' 1986 paper *"An O(ND) Difference Algorithm and Its Variations"*.
/// https://publications.mpi-cbg.de/Myers_1986_6330.pdf
///
/// Myers models diffing as finding the shortest path across a 2D edit graph where deletions
/// and insertions cost 1, while identical diagonal matches cost 0. By exploring along
/// diagonals `k = x - y`, it greedily tracks the furthest-reaching paths for each edit step.
///
/// This implementation maps Myers' linear-space refinement into an iterative architecture:
/// - `Workspace` hosts preallocated forward and backward diagonal buffers (`DiagonalBuffer` with `Index<isize>`),
/// - An explicit `EditBox` worklist replaces call-stack recursion to guarantee overflow safety,
/// - Bounding boxes bisect around bidirectional middle snakes until reaching base cases that coalesce into hunks.
///
/// # Arguments
/// * `len_a` - Number of elements in sequence A.
/// * `len_b` - Number of elements in sequence B.
/// * `equals` - An index-based equality predicate `equals(i, j)` returning `true` if element `i`
///   in sequence A equals element `j` in sequence B.
pub(crate) fn myers<F>(len_a: usize, len_b: usize, mut equals: F) -> EditScript
where
    F: FnMut(usize, usize) -> bool,
{
    let mut ws = Workspace::new(len_a + len_b);
    let mut hunks: Vec<Hunk> = Vec::new();

    // Stack for iterative divide-and-conquer (avoids call stack overflow on large inputs)
    let mut stack = Vec::with_capacity(32);
    stack.push(EditBox::new(0..len_a, 0..len_b));

    while let Some(mut edit_box) = stack.pop() {
        // Fast-path: trim common prefixes and suffixes
        edit_box.trim_common_prefix(&mut equals);
        edit_box.trim_common_suffix(&mut equals);

        if edit_box.is_empty() {
            continue;
        }

        // Base cases: one or both sequences exhausted
        if edit_box.old_len() == 0 || edit_box.new_len() == 0 {
            push_or_coalesce(&mut hunks, edit_box.into_hunk());
            continue;
        }

        // Bisect edit box around middle snake
        if let Some(snake) = middle_snake(&edit_box, &mut equals, &mut ws) {
            let (left, right) = edit_box.split_around(snake);
            // Push right box first so that left box is popped first.
            // This guarantees hunks are emitted in strictly ascending index order.
            stack.push(right);
            stack.push(left);
        } else {
            push_or_coalesce(&mut hunks, edit_box.into_hunk());
        }
    }

    EditScript::new(hunks)
}

/// Finds the middle snake in `O((N + M) * D)` time and `O(N + M)` space.
///
/// Explores forward D-paths from `(0, 0)` (the start of `edit_box`) and backward
/// D-paths from `(N, M)` (the end of `edit_box`, where `N = edit_box.old_len()`
/// and `M = edit_box.new_len()`) until they overlap on the same diagonal `k`.
/// Reuses `ws` without dynamic allocations.
fn middle_snake<F>(edit_box: &EditBox, equals: &mut F, ws: &mut Workspace) -> Option<Snake>
where
    F: FnMut(usize, usize) -> bool,
{
    let n = edit_box.old_len();
    let m = edit_box.new_len();
    let max = n + m;
    let delta = n as isize - m as isize;
    let odd = (delta & 1) != 0;

    let (vf, vb) = (&mut ws.vf, &mut ws.vb);

    // Initial endpoints: forward path at (0, 0) on diagonal k=0,
    // backward path at (n, m) on diagonal k=delta
    vf[0] = 0;
    vb[delta] = n;

    let d_max = max.div_ceil(2);
    let min_k = -(m as isize);
    let max_k = n as isize;

    for d in 1..=d_max {
        let d_isize = d as isize;

        // Forward search: explore D-paths from (0, 0)
        // Descending order (.rev().step_by(2)) preserves GNU diff tie-breaking preference
        for k in (-d_isize..=d_isize).rev().step_by(2) {
            if k < min_k || k > max_k {
                continue;
            }

            let x_start = if k == -d_isize
                || k == min_k
                || (k != d_isize && k != max_k && vf[k - 1] < vf[k + 1])
            {
                vf[k + 1]
            } else {
                vf[k - 1] + 1
            };

            let y_start_isize = x_start as isize - k;
            let (x0, y0) = (x_start, y_start_isize.max(0) as usize);

            let (x, _) = if y_start_isize >= 0 {
                extend_forward(
                    x_start,
                    y_start_isize as usize,
                    n,
                    m,
                    edit_box.old_range.start,
                    edit_box.new_range.start,
                    equals,
                )
            } else {
                (x_start, 0)
            };

            vf[k] = x; // Record the furthest-reaching forward endpoint on diagonal k

            // If delta is odd, check if forward search overlaps backward search
            if odd && k >= delta - (d_isize - 1) && k <= delta + (d_isize - 1) && vf[k] >= vb[k] {
                let snake_len = x - x0;
                return Some(Snake::new(x0, y0, snake_len));
            }
        }

        // Backward search: explore D-paths backwards from (N, M)
        for k in ((delta - d_isize)..=(delta + d_isize)).rev().step_by(2) {
            if k < min_k || k > max_k {
                continue;
            }

            let u_start = if k == delta + d_isize || k == max_k {
                vb[k - 1]
            } else if k == delta - d_isize || k == min_k {
                vb[k + 1].saturating_sub(1)
            } else if vb[k - 1] < vb[k + 1].saturating_sub(1) {
                vb[k - 1]
            } else {
                vb[k + 1].saturating_sub(1)
            };

            let v_start_isize = u_start as isize - k;
            let u0 = u_start;

            let (u, v) = if v_start_isize >= 0 {
                extend_backward(
                    u_start,
                    v_start_isize as usize,
                    edit_box.old_range.start,
                    edit_box.new_range.start,
                    equals,
                )
            } else {
                (u_start, 0)
            };
            vb[k] = u;

            // If delta is even, check if backward search overlaps forward search
            if !odd && k >= -d_isize && k <= d_isize && vf[k] >= vb[k] {
                let snake_len = u0 - u;
                return Some(Snake::new(u, v, snake_len));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_myers_identical() {
        let a = ["a", "b", "c"];
        let b = ["a", "b", "c"];
        let script = myers(a.len(), b.len(), |i, j| a[i] == b[j]);
        assert!(script.hunks().is_empty());
    }

    #[test]
    fn test_myers_delete_middle() {
        let a = ["a", "b", "c"];
        let b = ["a", "c"];
        let script = myers(a.len(), b.len(), |i, j| a[i] == b[j]);
        assert_eq!(script.hunks().len(), 1);
        assert_eq!(script.hunks()[0].old_range(), 1..2);
        assert_eq!(script.hunks()[0].new_range(), 1..1);
    }

    #[test]
    fn test_myers_insert_middle() {
        let a = ["a", "c"];
        let b = ["a", "b", "c"];
        let script = myers(a.len(), b.len(), |i, j| a[i] == b[j]);
        assert_eq!(script.hunks().len(), 1);
        assert_eq!(script.hunks()[0].old_range(), 1..1);
        assert_eq!(script.hunks()[0].new_range(), 1..2);
    }

    #[test]
    fn test_myers_complex() {
        // Myers 1986 paper example: A = "ABCABBA", B = "CBABAC"
        // Optimal D = 5: LCS is ["C", "B", "B", "A"] (length 4)
        let a = ["A", "B", "C", "A", "B", "B", "A"];
        let b = ["C", "B", "A", "B", "A", "C"];
        let script = myers(a.len(), b.len(), |i, j| a[i] == b[j]);

        assert_eq!(script.hunks().len(), 4);
        assert_eq!(script.hunks()[0].old_range(), 0..2); // Delete "A", "B"
        assert_eq!(script.hunks()[0].new_range(), 0..0);
        assert_eq!(script.hunks()[1].old_range(), 3..4); // Delete "A"
        assert_eq!(script.hunks()[1].new_range(), 1..1);
        assert_eq!(script.hunks()[2].old_range(), 5..5); // Insert "A"
        assert_eq!(script.hunks()[2].new_range(), 2..3);
        assert_eq!(script.hunks()[3].old_range(), 7..7); // Insert "C"
        assert_eq!(script.hunks()[3].new_range(), 5..6);
    }

    #[test]
    fn test_myers_both_empty() {
        let a: [&str; 0] = [];
        let b: [&str; 0] = [];
        let script = myers(a.len(), b.len(), |i, j| a[i] == b[j]);
        assert!(script.hunks().is_empty());
    }

    #[test]
    fn test_myers_empty_to_nonempty() {
        let a: [&str; 0] = [];
        let b = ["x", "y"];
        let script = myers(a.len(), b.len(), |i, j| a[i] == b[j]);
        assert_eq!(script.hunks().len(), 1);
        assert_eq!(script.hunks()[0].old_range(), 0..0);
        assert_eq!(script.hunks()[0].new_range(), 0..2);
    }

    #[test]
    fn test_myers_nonempty_to_empty() {
        let a = ["x", "y"];
        let b: [&str; 0] = [];
        let script = myers(a.len(), b.len(), |i, j| a[i] == b[j]);
        assert_eq!(script.hunks().len(), 1);
        assert_eq!(script.hunks()[0].old_range(), 0..2);
        assert_eq!(script.hunks()[0].new_range(), 0..0);
    }

    #[test]
    fn test_myers_completely_disjoint() {
        let a = ["1", "2", "3"];
        let b = ["4", "5", "6"];
        let script = myers(a.len(), b.len(), |i, j| a[i] == b[j]);
        assert_eq!(script.hunks().len(), 1);
        assert_eq!(script.hunks()[0].old_range(), 0..3);
        assert_eq!(script.hunks()[0].new_range(), 0..3);
    }

    #[test]
    fn test_myers_repeated_elements() {
        let a = ["a", "a", "a"];
        let b = ["a", "a"];
        let script = myers(a.len(), b.len(), |i, j| a[i] == b[j]);
        assert_eq!(script.hunks().len(), 1);
        assert_eq!(script.hunks()[0].old_range().len(), 1);
        assert_eq!(script.hunks()[0].new_range().len(), 0);
    }

    #[test]
    fn test_push_or_coalesce() {
        let mut hunks = Vec::new();
        // Adjacent deletion and insertion at the same position should merge into a replacement
        push_or_coalesce(&mut hunks, Hunk::new(1..2, 1..1)); // Delete index 1
        push_or_coalesce(&mut hunks, Hunk::new(2..2, 1..2)); // Insert at index 1
        push_or_coalesce(&mut hunks, Hunk::new(5..7, 4..4)); // Separate disjoint deletion

        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].old_range(), 1..2);
        assert_eq!(hunks[0].new_range(), 1..2);
        assert_eq!(hunks[1].old_range(), 5..7);
        assert_eq!(hunks[1].new_range(), 4..4);
    }

    #[test]
    fn test_myers_paper_1986() {
        // The example in Eugene W Myers paper
        let a = ["C", "B", "A", "B", "A", "C"];
        let b = ["A", "B", "C", "A", "B", "B", "A"];
        let script = myers(a.len(), b.len(), |i, j| a[i] == b[j]);
        // Optimal D = 5: matches B, A, B, A (length 4)
        assert_eq!(script.hunks().len(), 4);
        assert_eq!(script.hunks()[0].old_range(), 0..1); // Delete C
        assert_eq!(script.hunks()[0].new_range(), 0..1); // Replace with A
        assert_eq!(script.hunks()[1].old_range(), 2..2); // Insert C
        assert_eq!(script.hunks()[1].new_range(), 2..3);
        assert_eq!(script.hunks()[2].old_range(), 4..4); // Insert B
        assert_eq!(script.hunks()[2].new_range(), 5..6);
        assert_eq!(script.hunks()[3].old_range(), 5..6); // Delete C
        assert_eq!(script.hunks()[3].new_range(), 7..7);
    }

    #[test]
    fn test_myers_gnu_diff_alignment_fruits() {
        let a = ["banana", "mango", "carrot", "grapes", "apple"];
        let b = ["apple", "banana", "mango", "grapes", "carrot"];
        let script = myers(a.len(), b.len(), |i, j| a[i] == b[j]);
        // Matches banana, mango, grapes (LCS length 3, D = 4)
        assert_eq!(script.hunks().len(), 3);
        assert_eq!(script.hunks()[0].old_range(), 0..0); // Insert apple
        assert_eq!(script.hunks()[0].new_range(), 0..1);
        assert_eq!(script.hunks()[1].old_range(), 2..3); // Delete carrot
        assert_eq!(script.hunks()[1].new_range(), 3..3);
        assert_eq!(script.hunks()[2].old_range(), 4..5); // Replace apple with carrot
        assert_eq!(script.hunks()[2].new_range(), 4..5);
    }

    #[test]
    fn test_diagonal_buffer_indexing_and_mutation() {
        let mut buf = DiagonalBuffer::new(4);

        buf[-4] = 42;
        buf[0] = 100;
        buf[4] = 999;
        buf[-1] = buf[0] + 5;

        assert_eq!(buf[-4], 42);
        assert_eq!(buf[0], 100);
        assert_eq!(buf[4], 999);
        assert_eq!(buf[-1], 105);
    }

    #[test]
    fn test_workspace_buffers() {
        let mut ws = Workspace::new(5);
        ws.vf[0] = 10;
        ws.vf[-5] = 20;
        ws.vf[5] = 30;

        ws.vb[0] = 50;
        ws.vb[-5] = 60;
        ws.vb[5] = 70;

        assert_eq!(ws.vf[0], 10);
        assert_eq!(ws.vf[-5], 20);
        assert_eq!(ws.vf[5], 30);
        assert_eq!(ws.vb[0], 50);
        assert_eq!(ws.vb[-5], 60);
        assert_eq!(ws.vb[5], 70);
    }

    #[test]
    fn test_snake_properties() {
        let empty_snake = Snake::new(2, 3, 0);
        assert!(empty_snake.is_empty());
        assert_eq!(empty_snake.len, 0);
        assert_eq!(empty_snake.end_x(), 2);
        assert_eq!(empty_snake.end_y(), 3);

        let active_snake = Snake::new(1, 2, 3);
        assert!(!active_snake.is_empty());
        assert_eq!(active_snake.len, 3);
        assert_eq!(active_snake.end_x(), 4);
        assert_eq!(active_snake.end_y(), 5);
    }

    #[test]
    fn test_extend_forward_and_backward() {
        let a = ["a", "b", "c", "d"];
        let b = ["x", "b", "c", "y"];
        let mut equals = |i, j| a[i] == b[j];

        // Forward extension: starting at (1, 1), offset (0, 0)
        let (x, y) = extend_forward(1, 1, 4, 4, 0, 0, &mut equals);
        assert_eq!((x, y), (3, 3));

        // Backward extension: starting backwards from (3, 3), offset (0, 0)
        let (x, y) = extend_backward(3, 3, 0, 0, &mut equals);
        assert_eq!((x, y), (1, 1));
    }

    #[test]
    fn test_edit_box_trimming_and_splitting() {
        let a = ["header", "a", "b", "footer"];
        let b = ["header", "x", "footer"];
        let mut equals = |i, j| a[i] == b[j];

        let mut box_ = EditBox::new(0..a.len(), 0..b.len());
        assert_eq!(box_.trim_common_prefix(&mut equals), 1);
        assert_eq!(box_.old_range, 1..4);
        assert_eq!(box_.new_range, 1..3);

        assert_eq!(box_.trim_common_suffix(&mut equals), 1);
        assert_eq!(box_.old_range, 1..3); // "a", "b"
        assert_eq!(box_.new_range, 1..2); // "x"

        // Split around a snake at relative (1, 0) of len 0
        let snake = Snake::new(1, 0, 0);
        let (left, right) = box_.split_around(snake);
        assert_eq!(left.old_range, 1..2);
        assert_eq!(left.new_range, 1..1);
        assert_eq!(right.old_range, 2..3);
        assert_eq!(right.new_range, 1..2);
    }

    #[test]
    fn test_edit_box_into_hunk() {
        let box_ = EditBox::new(2..5, 3..3);
        let hunk = box_.into_hunk();
        assert_eq!(hunk.old_range(), 2..5);
        assert_eq!(hunk.new_range(), 3..3);
        assert!(hunk.new_range().is_empty());
    }
}
