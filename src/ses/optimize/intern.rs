// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::ses::optimize::fxhasher::{FxBuildHasher, FxHashMap};
use core::hash::{Hash, Hasher};

/// Internal key wrapper whose `Hash` implementation writes slice bytes directly,
/// avoiding `std`'s extra `write_usize(len)` prefix call since [`crate::ses::optimize::fxhasher::FxHasher`]
/// already mixes the slice length inside `hash_bytes`. This is just a small micro-optimization.
#[derive(Eq, PartialEq)]
struct LineKey<'a>(&'a [u8]);

impl Hash for LineKey<'_> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.0);
    }
}

/// Maps arbitrary byte sequences to unique `u32` token IDs.
///
/// Uses [`FxHashMap`] for fast, non-cryptographic hashing of short byte slices
/// (such as source code lines), avoiding the latency of standard cryptographic hashing
/// while guaranteeing sound equality checks without hash collisions.
pub(crate) struct TokenInterner<'a> {
    map: FxHashMap<LineKey<'a>, u32>,
}

impl<'a> TokenInterner<'a> {
    /// Creates a new `TokenInterner` with pre-allocated capacity for `capacity` unique tokens,
    /// eliminating dynamic table reallocations and rehashing.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher),
        }
    }

    /// Interns a slice of tokens implementing `AsRef<[u8]>` into a `Vec<u32>`, mapping identical
    /// tokens to matching incremental `u32` IDs.
    pub fn intern<T: AsRef<[u8]>>(&mut self, tokens: &'a [T]) -> Vec<u32> {
        let mut ids = Vec::with_capacity(tokens.len());
        for tok in tokens {
            let bytes: &'a [u8] = tok.as_ref();
            let next_id = self.map.len() as u32;
            let key = LineKey(bytes);
            let id = *self.map.entry(key).or_insert(next_id);
            ids.push(id);
        }
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interner() {
        let tokens = ["apple", "banana", "apple", "cherry"];
        let mut interner = TokenInterner::with_capacity(tokens.len());
        let ids = interner.intern(&tokens);

        assert_eq!(ids.len(), 4);
        assert_eq!(ids[0], ids[2]); // "apple" == "apple"
        assert_ne!(ids[0], ids[1]); // "apple" != "banana"
        assert_ne!(ids[1], ids[3]); // "banana" != "cherry"
    }

    #[test]
    fn test_interner_raw_bytes() {
        let tokens: [&[u8]; 4] = [b"apple", b"banana", b"apple", b"cherry"];
        let mut interner = TokenInterner::with_capacity(tokens.len());
        let ids = interner.intern(&tokens);

        assert_eq!(ids.len(), 4);
        assert_eq!(ids[0], ids[2]); // b"apple" == b"apple"
        assert_ne!(ids[0], ids[1]); // b"apple" != b"banana"
        assert_ne!(ids[1], ids[3]); // b"banana" != b"cherry"
    }

    #[test]
    fn test_interner_empty_and_zero_capacity() {
        let mut interner = TokenInterner::with_capacity(0);
        let empty_tokens: [&str; 0] = [];
        let ids = interner.intern(&empty_tokens);
        assert!(ids.is_empty());

        // Intern empty strings/bytes
        let empty_str = ["", ""];
        let ids2 = interner.intern(&empty_str);
        assert_eq!(ids2.len(), 2);
        assert_eq!(ids2[0], ids2[1]);
    }

    #[test]
    fn test_interner_chunk_boundary_differences() {
        let mut interner = TokenInterner::with_capacity(16);
        // Vary lengths around 8-byte word boundaries: 7, 8, 9, 15, 16, 17 bytes
        let tokens = [
            &b"1234567"[..],
            &b"1234568"[..],           // differs at 7th byte
            &b"12345678"[..],          // 8 bytes
            &b"12345679"[..],          // 8 bytes, differs at 8th byte
            &b"123456789"[..],         // 9 bytes
            &b"123456780"[..],         // 9 bytes, differs at 9th byte
            &b"123456781234567"[..],   // 15 bytes
            &b"1234567812345678"[..],  // 16 bytes
            &b"1234567812345679"[..],  // 16 bytes, differs at last byte
            &b"12345678123456780"[..], // 17 bytes
        ];
        let ids = interner.intern(&tokens);
        assert_eq!(ids.len(), 10);
        // All these must have distinct IDs
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "token {} and {} had collision", i, j);
            }
        }
    }

    #[test]
    fn test_interner_incremental_batches() {
        let mut interner = TokenInterner::with_capacity(4);
        let batch1 = ["alpha", "beta"];
        let ids1 = interner.intern(&batch1);

        let batch2 = ["gamma", "alpha", "beta", "delta"];
        let ids2 = interner.intern(&batch2);

        assert_eq!(ids1[0], ids2[1]); // "alpha"
        assert_eq!(ids1[1], ids2[2]); // "beta"
        assert_ne!(ids2[0], ids1[0]); // "gamma" is new
        assert_ne!(ids2[3], ids1[0]); // "delta" is new
    }

    #[test]
    fn test_interner_long_byte_lines() {
        let mut interner = TokenInterner::with_capacity(4);
        let long1 = vec![b'x'; 2000];
        let mut long2 = vec![b'x'; 2000];
        long2[1999] = b'y'; // differs at byte 1999

        let tokens = [long1.as_slice(), long2.as_slice(), long1.as_slice()];
        let ids = interner.intern(&tokens);

        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0], ids[2]);
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn test_interner_trailing_nul_and_length_distinctness() {
        let mut interner = TokenInterner::with_capacity(8);
        let tokens: [&[u8]; 5] = [b"", b"\0", b"\0\0", b"line", b"line\0"];
        let ids = interner.intern(&tokens);
        assert_eq!(ids.len(), 5);
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "index {i} and {j} unexpectedly matched");
            }
        }
    }
}
