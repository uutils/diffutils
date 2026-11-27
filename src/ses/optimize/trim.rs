// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

/// Stores the results of common prefix and suffix stripping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Trimmed<'a, T> {
    pub prefix_len: usize,
    pub suffix_len: usize,
    pub slice_a: &'a [T],
    pub slice_b: &'a [T],
}

/// Strips common prefix and suffix elements in `O(N)` time before running the Myers engine.
pub(crate) fn trim_slices<'a, T, F>(a: &'a [T], b: &'a [T], mut equals: F) -> Trimmed<'a, T>
where
    F: FnMut(&T, &T) -> bool,
{
    let mut prefix_len = 0;
    while prefix_len < a.len() && prefix_len < b.len() && equals(&a[prefix_len], &b[prefix_len]) {
        prefix_len += 1;
    }

    let mut suffix_len = 0;
    while suffix_len < (a.len() - prefix_len)
        && suffix_len < (b.len() - prefix_len)
        && equals(&a[a.len() - 1 - suffix_len], &b[b.len() - 1 - suffix_len])
    {
        suffix_len += 1;
    }

    Trimmed {
        prefix_len,
        suffix_len,
        slice_a: &a[prefix_len..a.len() - suffix_len],
        slice_b: &b[prefix_len..b.len() - suffix_len],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_slices() {
        let a = ["header", "a", "footer"];
        let b = ["header", "b", "footer"];
        let trimmed = trim_slices(&a, &b, |x, y| x == y);

        assert_eq!(trimmed.prefix_len, 1);
        assert_eq!(trimmed.suffix_len, 1);
        assert_eq!(trimmed.slice_a, &["a"]);
        assert_eq!(trimmed.slice_b, &["b"]);
    }

    #[test]
    fn test_trim_identical() {
        let a = ["header", "body", "footer"];
        let b = ["header", "body", "footer"];
        let trimmed = trim_slices(&a, &b, |x, y| x == y);

        assert_eq!(trimmed.prefix_len, 3);
        assert_eq!(trimmed.suffix_len, 0);
        assert!(trimmed.slice_a.is_empty());
        assert!(trimmed.slice_b.is_empty());
    }

    #[test]
    fn test_trim_disjoint() {
        let a = ["1", "2"];
        let b = ["3", "4"];
        let trimmed = trim_slices(&a, &b, |x, y| x == y);

        assert_eq!(trimmed.prefix_len, 0);
        assert_eq!(trimmed.suffix_len, 0);
        assert_eq!(trimmed.slice_a, &["1", "2"]);
        assert_eq!(trimmed.slice_b, &["3", "4"]);
    }
}
