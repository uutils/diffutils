// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

const BITS_PER_WORD: usize = 64;

/// A compact fixed-capacity bitset backed by packed `u64` words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bitset {
    words: Vec<u64>,
    len: usize,
}

impl Bitset {
    /// Creates a new `Bitset` of `len` bits initialized to `false`.
    pub fn new(len: usize) -> Self {
        let words_required = len.div_ceil(BITS_PER_WORD);
        Bitset {
            words: vec![0; words_required],
            len,
        }
    }

    /// Returns the total number of bits.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the bitset contains zero bits.
    #[cfg(test)]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    fn get_word_and_mask(index: usize) -> (usize, u64) {
        let word_index = index / BITS_PER_WORD;
        let bit_mask = 1u64 << (index % BITS_PER_WORD);
        (word_index, bit_mask)
    }

    /// Returns the bit value at `index`.
    #[inline]
    pub fn get(&self, index: usize) -> bool {
        debug_assert!(index < self.len);
        let (word_index, mask) = Bitset::get_word_and_mask(index);
        (self.words[word_index] & mask) != 0
    }

    /// Sets the bit at `index` to `1` (true).
    #[inline]
    pub fn set(&mut self, index: usize) {
        debug_assert!(index < self.len);
        let (word_index, mask) = Bitset::get_word_and_mask(index);
        self.words[word_index] |= mask;
    }

    /// Clears the bit at `index` to `0` (false).
    #[inline]
    pub fn clear(&mut self, index: usize) {
        debug_assert!(index < self.len);
        let (word_index, mask) = Bitset::get_word_and_mask(index);
        self.words[word_index] &= !mask;
    }

    /// Sets all bits in `range` to `1` (true).
    pub fn set_range(&mut self, range: std::ops::Range<usize>) {
        debug_assert!(range.end <= self.len);
        for i in range {
            self.set(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitset_empty() {
        let bs = Bitset::new(0);
        assert_eq!(bs.len(), 0);
        assert!(bs.is_empty());
        assert_eq!(bs.words.len(), 0);
    }

    #[test]
    fn test_bitset_set_and_get() {
        let mut bs = Bitset::new(130);
        assert_eq!(bs.len(), 130);
        assert_eq!(bs.words.len(), 3); // 130 / 64 rounded up is 3

        // Initially all bits are false
        for i in 0..130 {
            assert!(!bs.get(i));
        }

        // Set boundary bits: 0, 63, 64, 127, 129
        bs.set(0);
        bs.set(63);
        bs.set(64);
        bs.set(127);
        bs.set(129);

        assert!(bs.get(0));
        assert!(bs.get(63));
        assert!(bs.get(64));
        assert!(bs.get(127));
        assert!(bs.get(129));

        // Neighboring bits must remain false
        assert!(!bs.get(1));
        assert!(!bs.get(62));
        assert!(!bs.get(65));
        assert!(!bs.get(126));
        assert!(!bs.get(128));
    }

    #[test]
    fn test_bitset_clear() {
        let mut bs = Bitset::new(100);
        bs.set(42);
        assert!(bs.get(42));

        bs.clear(42);
        assert!(!bs.get(42));
    }

    #[test]
    fn test_bitset_set_range() {
        let mut bs = Bitset::new(200);

        // Empty range does nothing
        bs.set_range(10..10);
        assert!(!bs.get(10));
        bs.set_range(0..0);

        // Range within single word
        bs.set_range(5..10);
        for i in 0..200 {
            assert_eq!(bs.get(i), (5..10).contains(&i));
        }

        // Cross word boundary (64)
        bs.set_range(60..70);
        for i in 5..10 {
            assert!(bs.get(i));
        }
        for i in 60..70 {
            assert!(bs.get(i));
        }
        assert!(!bs.get(59));
        assert!(!bs.get(70));

        // Exact single word (64..128)
        bs.set_range(64..128);
        for i in 64..128 {
            assert!(bs.get(i));
        }

        // Spanning across multiple words (10..150)
        let mut bs2 = Bitset::new(200);
        bs2.set_range(10..150);
        for i in 0..200 {
            assert_eq!(bs2.get(i), (10..150).contains(&i));
        }
    }

    #[test]
    fn test_bitset_clone_and_eq() {
        let mut bs1 = Bitset::new(70);
        bs1.set(10);
        bs1.set(65);

        let bs2 = bs1.clone();
        assert_eq!(bs1, bs2);

        let mut bs3 = bs1.clone();
        bs3.set(11);
        assert_ne!(bs1, bs3);
    }

    #[test]
    fn test_bitset_words_allocation() {
        assert_eq!(Bitset::new(0).words.len(), 0);
        assert_eq!(Bitset::new(1).words.len(), 1);
        assert_eq!(Bitset::new(63).words.len(), 1);
        assert_eq!(Bitset::new(64).words.len(), 1);
        assert_eq!(Bitset::new(65).words.len(), 2);
        assert_eq!(Bitset::new(128).words.len(), 2);
        assert_eq!(Bitset::new(129).words.len(), 3);
        assert_eq!(Bitset::new(100_000).words.len(), 1563);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn test_bitset_get_out_of_bounds() {
        let bs = Bitset::new(10);
        let _ = bs.get(10);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn test_bitset_set_out_of_bounds() {
        let mut bs = Bitset::new(10);
        bs.set(10);
    }
}
