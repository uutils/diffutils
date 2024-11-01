// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

// This engine implements the Myers diff algorithm, which uses a double-ended
// diagonal search to identify the longest common subsequence (LCS) between two
// collections. The original paper can be found here:
//
// https://link.springer.com/article/10.1007/BF01840446
//
// Unlike a naive LCS implementation, which covers all possible combinations,
// the Myers algorithm gradualy expands the search space, and only encodes
// the furthest progress made by each diagonal rather than storing each step
// of the search on a matrix.
//
// This makes it a lot more memory-efficient, as it only needs 2 * (m + n)
// positions to represent the state of the search, where m and n are the number
// of items in the collections being compared, whereas the naive LCS requires
// m * n positions.
//
// The downside is it is more compute-intensive than the naive method when
// searching through very different files. This may lead to unnacceptable run
// time in pathological cases (large, completely different files), so heuristics
// are often used to bail on the search if it gets too costly and/or a good enough
// subsequence has been found.
//
// We implement 3 main heuristics that are also used by GNU diff:
//
// 1. if we found a large enough common subsequence (also known as a 'snake')
// and have searched for a while, we return that one
//
// 2. if we have searched for a significant chunk of the collections (with a
// minimum of 4096 iterations, so we cover easy cases fully) and have not found
// one, we use whatever we have, even if it is a small snake or no snake at all
//
// 3. we keep track of the overall cost of the various searches that are done
// over the course of the divide and conquer strategy, and if that becomes too
// large we give up on trying to find long similarities altogether
//
// This last heuristic could be improved significantly in the future if we
// implement an optimization that separates items that only appear in either
// collection and remove them from the diffing process, like GNU diff does.
use std::fmt::Debug;
use std::ops::{Index, IndexMut, RangeInclusive};

use rand::Rng as _;
use tracing::{info, instrument, trace, Level};

#[derive(Debug, Default, PartialEq)]
struct Snake {
    x: usize,
    y: usize,
    length: usize,
}

impl Snake {
    fn is_good(&self) -> bool {
        // This magic number comes from GNU diff.
        self.length > 20
    }

    fn maybe_update(&mut self, x: isize, y: isize, length: isize) {
        let length = length as usize;
        if length > self.length {
            trace!(x = x, y = y, length = length, "new best snake");
            self.x = x as usize;
            self.y = y as usize;
            self.length = length;
        }
    }

    fn maybe_set(&mut self, x: isize, y: isize) {
        if self.length == 0 {
            self.x = x as usize;
            self.y = y as usize;
        }
    }
}

#[instrument(skip_all)]
fn find_split_point<T: Clone + Debug + PartialEq + Into<Vec<u8>>>(
    left: &[T],
    right: &[T],
    total_cost: &mut usize,
) -> Snake {
    let left_length = left.len() as isize;
    let right_length = right.len() as isize;

    let max_cost = left_length + right_length;

    // This constant is the value used by GNU diff; using it should give us
    // more similar diffs.
    const HIGH_COST: isize = 200;

    // This magic number was borrowed from GNU diff - apparently this is a
    // good number for modern CPUs.
    let too_expensive: isize = ((max_cost as f64).sqrt() as isize).max(4096);
    info!(too_expensive = too_expensive);

    // We've been constantly hitting the too expensive heuristic, this means the
    // files are too different for us to get a good diff in reasonable amount of
    // time. Do naive splits from now on.
    if *total_cost as isize > too_expensive * 10 {
        info!(
            total_cost = total_cost,
            "hit too costly overall heuristic, creating naive split"
        );
        let mut rng = rand::thread_rng();
        let x = if left_length == 0 {
            0
        } else {
            rng.gen_range(0..left.len())
        };
        let y = if right_length == 0 {
            0
        } else {
            rng.gen_range(0..right.len())
        };
        return Snake { x, y, length: 0 };
    }

    // For collections of different sizes, the diagonals will not neatly balance. That means the
    // "middle" diagonal for the backwards search will be offset from the forward one, so we need
    // to keep track of that so we start at the right point.
    let backwards_mid = left_length - right_length;

    // Since we explore in steps of 2, if the offset mentioned above is odd the diagonals will
    // not align during exploration. We use this to know if we check for meeting in the middle
    // in the forwards or backwards search.
    let ends_align = backwards_mid & 1 != 1;

    trace!(backwards_mid = backwards_mid, ends_align = ends_align);

    // The diagonals are initialized with values that are outside of the limits of the expected
    // values so that the edit choices at the frontiers are always correct. We set the values at
    // the mid diagonals to their correct initial values, though.
    //
    // The conceptual model of this algorithm is that 'left' is the title row of a matrix, and
    // 'right' is the title column. The vector positions represent the best value of x we managed
    // to achieve so far for each of those diagonals, rather then filling in the whole matrix. Note
    // that "best" will be "the highest" for forward searches, and "the lowest" for backward, since
    // we start from the high end on that one.
    //
    // Let's focus on the forward one, with x as an index for 'left', y as index for 'right', and
    // d as the index of the vector. At the start, d = 0 means x = 0, y = 0, no offsets. If we go
    // to the previous position on the vector, that conceptually means increasing the offset of y,
    // since its value is derived from x - d. Offsetting 'right' means we are exploring an insertion.
    // Going to the next position on the other hand means we are decreasing the offset of y, which
    // means we are exploring a deletion (offsetting x relative to y).
    let mut forward_diagonals = Diagonals::new(-1isize, left_length, right_length);
    forward_diagonals[0] = 0;

    let mut backward_diagonals = Diagonals::new(isize::MAX, left_length, right_length);
    backward_diagonals[backwards_mid] = left_length;

    let in_bounds = |x: isize, y: isize, offset: isize| -> bool {
        x >= offset && y >= offset && x < left_length + offset && y < right_length + offset
    };

    let mut best_snake = Snake::default();

    let forward_span = tracing::span!(Level::TRACE, "forward");
    let backward_span = tracing::span!(Level::TRACE, "backward");
    'outer: for c in 1..max_cost {
        *total_cost += 1;

        info!(c = c, snake_length = best_snake.length);
        // The files appear to be large and too different. Go for good enough
        if c > too_expensive {
            break 'outer;
        }

        // Forwards search
        forward_diagonals.expand_search();
        let fwd = forward_span.enter();
        trace!(
            low = forward_diagonals.search_range.start(),
            high = forward_diagonals.search_range.end(),
            "search space"
        );
        for d in forward_diagonals.search_range.clone().rev().step_by(2) {
            let mut x = if forward_diagonals[d - 1] < forward_diagonals[d + 1] {
                trace!(
                    insertion = forward_diagonals[d - 1],
                    deletion = forward_diagonals[d + 1],
                    "exploring deletion"
                );
                forward_diagonals[d + 1]
            } else {
                trace!(
                    insertion = forward_diagonals[d - 1],
                    deletion = forward_diagonals[d + 1],
                    "exploring insertion"
                );
                forward_diagonals[d - 1] + 1
            };
            debug_assert!(x != -1);

            let initial_x = x;
            let mut y = x - d;

            trace!(d = d, x = x, y = y, "before snaking");
            while in_bounds(x, y, 0) && left[x as usize] == right[y as usize] {
                x += 1;
                y += 1;
            }
            trace!(d = d, x = x, y = y, "after snaking");

            forward_diagonals[d] = x;

            let snake_length = x - initial_x;
            best_snake.maybe_update(initial_x, y - snake_length, snake_length);

            if !ends_align
                && backward_diagonals.search_range.contains(&d)
                && x >= backward_diagonals[d]
            {
                trace!("met backward at mid point");
                best_snake.maybe_set(x, y);
                break 'outer;
            }
        }
        drop(fwd);

        // Backwards search
        backward_diagonals.expand_search();
        let bwd = backward_span.enter();
        trace!(
            low = backward_diagonals.search_range.start(),
            high = backward_diagonals.search_range.end(),
            "search space"
        );
        for d in backward_diagonals.search_range.clone().rev().step_by(2) {
            // If we hit this assert we went outside the explored boundaries.
            debug_assert!(
                backward_diagonals[d - 1] != isize::MAX || backward_diagonals[d + 1] != isize::MAX
            );
            let mut x = if backward_diagonals[d - 1] < backward_diagonals[d + 1] {
                trace!(
                    insertion = backward_diagonals[d - 1],
                    deletion = backward_diagonals[d + 1],
                    "exploring insertion"
                );
                backward_diagonals[d - 1]
            } else {
                trace!(
                    insertion = backward_diagonals[d - 1],
                    deletion = backward_diagonals[d + 1],
                    "exploring deletion"
                );
                backward_diagonals[d + 1] - 1
            };

            let initial_x = x;
            let mut y = x - d;

            trace!(d = d, x = x, y = y, "before snaking");
            while in_bounds(x, y, 1) && left[x as usize - 1] == right[y as usize - 1] {
                x -= 1;
                y -= 1;
            }
            trace!(d = d, x = x, y = y, "after snaking");

            backward_diagonals[d] = x;

            best_snake.maybe_update(x, y, initial_x - x);

            if ends_align
                && forward_diagonals.search_range.contains(&d)
                && x <= forward_diagonals[d]
            {
                trace!("met forward at mid point");
                best_snake.maybe_set(x, y);
                break 'outer;
            }
        }
        drop(bwd);

        if c > HIGH_COST && best_snake.is_good() {
            info!("met criteria for high cost with good snake heuristic");
            break 'outer;
        }
    }

    // If we hit this condition, the search ran too long and found 0 matches.
    // Get the best we can do as a split point - furthest diagonal.
    if best_snake.length == 0 {
        let (x, y) = forward_diagonals.get_furthest_progress();
        best_snake.x = x;
        best_snake.y = y;
    }

    info!(
        x = best_snake.x,
        y = best_snake.y,
        length = best_snake.length,
        "** DONE best snake:"
    );
    best_snake
}

// Delete: we skip that line from 'left'
// Insert: we add that line from 'right'
// Keep: both have that line, leave untouched
#[derive(Debug)]
pub enum Edit<'a, T: Debug + PartialEq> {
    Delete(&'a T),
    Insert(&'a T),
    Keep(&'a T),
}

#[instrument(skip_all)]
pub fn diff<'a, T: Clone + Debug + PartialEq + Into<Vec<u8>>>(
    left: &'a [T],
    right: &'a [T],
) -> Vec<Edit<'a, T>> {
    trace!(left_length = left.len(), right_length = right.len());
    let mut edits = vec![];
    let mut total_cost = 0;
    do_diff(left, right, &mut edits, &mut total_cost);
    edits
}

#[instrument(skip_all)]
fn do_diff<'a, T: Clone + Debug + PartialEq + Into<Vec<u8>>>(
    left: &'a [T],
    right: &'a [T],
    edits: &mut Vec<Edit<'a, T>>,
    total_cost: &mut usize,
) {
    if left.is_empty() {
        right.iter().for_each(|r| edits.push(Edit::Insert(r)));
        return;
    } else if right.is_empty() {
        left.iter().for_each(|l| edits.push(Edit::Delete(l)));
        return;
    }

    // Add leading matches to our edits while finding them.
    let leading_matches = left
        .iter()
        .zip(right.iter())
        .take_while(|(l, r)| l == r)
        .map(|(l, _)| edits.push(Edit::Keep(l)))
        .count();

    // We need to hold on to add the trailing ones to keep ordering
    // so just calculate how many there are.
    let trailing_matches = left[leading_matches..]
        .iter()
        .rev()
        .zip(right[leading_matches..].iter().rev())
        .take_while(|(l, r)| l == r)
        .count();

    trace!(
        leading_matches = leading_matches,
        trailing_matches = trailing_matches
    );

    let left_remaining = &left[leading_matches..left.len() - trailing_matches];
    let right_remaining = &right[leading_matches..right.len() - trailing_matches];

    let snake = find_split_point(left_remaining, right_remaining, total_cost);

    trace!(x = snake.x, y = snake.y, length = snake.length, "snake");

    // No more matches were found, do all deletions / insertions.
    if snake.length == 0 {
        left_remaining
            .iter()
            .for_each(|l| edits.push(Edit::Delete(l)));
        right_remaining
            .iter()
            .for_each(|r| edits.push(Edit::Insert(r)));
    } else {
        // Divide and conquer based on the best snake we found.
        let (l1, l2) = left_remaining.split_at(snake.x);
        let (r1, r2) = right_remaining.split_at(snake.y);

        trace!(
            a = l1.len(),
            b = r1.len(),
            a = l2.len(),
            b = r2.len(),
            "split"
        );

        do_diff(l1, r1, edits, total_cost);
        do_diff(l2, r2, edits, total_cost);
    }

    // Finally add the trailing matches.
    left[left.len() - trailing_matches..]
        .iter()
        .for_each(|l| edits.push(Edit::Keep(l)));
}

struct Diagonals {
    data: Vec<isize>,
    center: usize,
    search_range: RangeInclusive<isize>,

    min_diag: isize,
    max_diag: isize,
}

impl Debug for Diagonals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, v) in self.data[..self.center].iter().enumerate() {
            let _ = write!(f, "({}: {v})", i as isize - self.center as isize);
        }

        let _ = writeln!(f, "\ncenter: ({}: {})", self.center, self.data[self.center]);

        for (i, v) in self.data[self.center + 1..].iter().enumerate() {
            let _ = write!(f, "({}: {v})", i + 1);
        }

        Ok(())
    }
}

impl Diagonals {
    pub fn new(filler: isize, left_length: isize, right_length: isize) -> Self {
        let size = left_length
            .checked_add(right_length)
            .and_then(|s| s.checked_add(3));
        let Some(size) = size else {
            panic!("Tried to create Diagonals of a size we cannot represent: {left_length} + {right_length} + 3");
        };

        // Our internal representaiton has 3 more positions than the sum of the lengths.
        // That is because we always look at diagonals of either side when evaluating a
        // diagonal, so we need room for an "out of bounds" value when checking the extremes.
        // We also need room to represent the middle diagonal at the middle.
        let mid_diag = left_length - right_length;
        Self {
            data: vec![filler; size as usize],
            center: (right_length + 1) as usize,
            search_range: mid_diag..=mid_diag,

            min_diag: -(right_length),
            max_diag: left_length,
        }
    }

    fn actual_index(&self, index: isize) -> usize {
        (self.center as isize + index) as usize
    }

    fn in_bounds(&self, index: isize) -> bool {
        let actual = self.center as isize + index;
        actual >= 0 && (actual as usize) < self.data.len()
    }

    fn get_furthest_progress(&self) -> (usize, usize) {
        let (d, x) = self
            .data
            .iter()
            .enumerate()
            .filter(|(d, &x)| x - (*d as isize) >= 0)
            .max_by_key(|(_, &x)| x)
            .map(|(i, x)| (i as isize, *x))
            .unwrap_or((0isize, 0isize));
        let y = x - d;
        debug_assert!(x >= 0);
        debug_assert!(y >= 0);
        (x as usize, y as usize)
    }

    fn expand_search(&mut self) {
        let upper = if *self.search_range.end() == self.max_diag {
            self.search_range.end() - 1
        } else {
            self.search_range.end() + 1
        };
        let lower = (self.search_range.start() - 1).max(self.min_diag);

        trace!(
            min_diag = self.min_diag,
            max_diag = self.max_diag,
            prev_lower = self.search_range.start(),
            prev_upper = self.search_range.end(),
            new_lower = lower,
            new_upper = upper,
        );

        self.search_range = lower..=upper;
    }
}

impl Index<isize> for Diagonals {
    type Output = isize;

    fn index(&self, index: isize) -> &Self::Output {
        if !self.in_bounds(index) {
            panic!("Index out of bounds: {} for SignedVec", index);
        }
        let actual_index = self.actual_index(index);
        &self.data[actual_index]
    }
}

impl IndexMut<isize> for Diagonals {
    fn index_mut(&mut self, index: isize) -> &mut Self::Output {
        if !self.in_bounds(index) {
            panic!("Index out of bounds: {} for SignedVec", index);
        }
        let actual_index = self.actual_index(index);
        &mut self.data[actual_index]
    }
}
