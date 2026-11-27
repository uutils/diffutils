// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

//! Fast, non-cryptographic hashing for internal token interning.
//!
//! Derived from `rustc-hash` v2 (`Apache-2.0 OR MIT`, Orson Peters / The Rust Project Developers):
//! polynomial `usize` accumulation paired with a wyhash-inspired byte-slice compressor
//! (`hash_bytes`) and bit-rotation finalizer tailored for `std::collections::HashMap` (`hashbrown`).
//!
//! Do NOT use for HashDoS-sensitive untrusted public hash tables; `TokenInterner` always verifies
//! collisions via full byte-slice equality.

use core::hash::{BuildHasher, Hasher};

#[cfg(target_pointer_width = "64")]
const K: usize = 0xf135_7aea_2e62_a9c5;
#[cfg(target_pointer_width = "32")]
const K: usize = 0x93d7_65dd;

const SEED1: u64 = 0x243f_6a88_85a3_08d3;
const SEED2: u64 = 0x1319_8a2e_0370_7344;
const PREVENT_TRIVIAL_ZERO_COLLAPSE: u64 = 0xa409_3822_299f_31d0;

/// Type alias for a `std::collections::HashMap` using the in-tree [`FxBuildHasher`].
pub(crate) type FxHashMap<K, V> = std::collections::HashMap<K, V, FxBuildHasher>;

/// A `BuildHasher` constructing default [`FxHasher`] instances.
#[derive(Copy, Clone, Default)]
pub(crate) struct FxBuildHasher;

impl BuildHasher for FxBuildHasher {
    type Hasher = FxHasher;

    #[inline]
    fn build_hasher(&self) -> FxHasher {
        FxHasher::new()
    }
}

/// Non-cryptographic, high-throughput `Hasher` for short keys and byte slices.
#[derive(Clone, Default)]
pub(crate) struct FxHasher {
    hash: usize,
}

impl FxHasher {
    /// Creates a new `FxHasher` initialized with seed `0`.
    #[inline]
    pub(crate) const fn new() -> Self {
        Self { hash: 0 }
    }

    /// Creates a new `FxHasher` initialized with a custom `seed`.
    #[cfg(test)]
    #[inline]
    pub(crate) const fn with_seed(seed: usize) -> Self {
        Self { hash: seed }
    }

    #[inline]
    fn add_to_hash(&mut self, i: usize) {
        self.hash = self.hash.wrapping_add(i).wrapping_mul(K);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.write_u64(hash_bytes(bytes));
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i as usize);
        #[cfg(target_pointer_width = "32")]
        self.add_to_hash((i >> 32) as usize);
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.add_to_hash(i as usize);
        #[cfg(target_pointer_width = "32")]
        self.add_to_hash((i >> 32) as usize);
        self.add_to_hash((i >> 64) as usize);
        #[cfg(target_pointer_width = "32")]
        self.add_to_hash((i >> 96) as usize);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i);
    }

    #[inline]
    fn finish(&self) -> u64 {
        #[cfg(target_pointer_width = "64")]
        const ROTATE: u32 = 26;
        #[cfg(target_pointer_width = "32")]
        const ROTATE: u32 = 15;

        // Rotate high-entropy MSB bits down for bucket indexing while keeping
        // strong entropy in the upper 7 bits used by `hashbrown` SIMD control bytes.
        self.hash.rotate_left(ROTATE) as u64
    }
}

#[inline]
fn multiply_mix(x: u64, y: u64) -> u64 {
    if cfg!(any(
        all(
            target_pointer_width = "64",
            not(any(target_arch = "sparc64", target_arch = "wasm64")),
        ),
        target_arch = "aarch64",
        target_arch = "x86_64",
        all(target_family = "wasm", target_feature = "wide-arithmetic"),
    )) {
        let full = (x as u128).wrapping_mul(y as u128);
        let lo = full as u64;
        let hi = (full >> 64) as u64;
        lo ^ hi
    } else {
        let lx = x as u32;
        let ly = y as u32;
        let hx = (x >> 32) as u32;
        let hy = (y >> 32) as u32;

        let afull = (lx as u64).wrapping_mul(hy as u64);
        let bfull = (hx as u64).wrapping_mul(ly as u64);
        afull ^ bfull.rotate_right(32)
    }
}

#[inline]
fn hash_bytes(bytes: &[u8]) -> u64 {
    let len = bytes.len();
    let mut s0 = SEED1;
    let mut s1 = SEED2;

    if len <= 16 {
        if len >= 8 {
            s0 ^= u64::from_le_bytes(bytes[0..8].try_into().unwrap());
            s1 ^= u64::from_le_bytes(bytes[len - 8..].try_into().unwrap());
        } else if len >= 4 {
            s0 ^= u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as u64;
            s1 ^= u32::from_le_bytes(bytes[len - 4..].try_into().unwrap()) as u64;
        } else if len > 0 {
            let lo = bytes[0];
            let mid = bytes[len / 2];
            let hi = bytes[len - 1];
            s0 ^= lo as u64;
            s1 ^= ((hi as u64) << 8) | mid as u64;
        }
    } else {
        // Iterate up to `len - 1` so exact multiples of 16 do not hash the final
        // 16 bytes twice when mixing the overlapping suffix below.
        let mut bulk = &bytes[..(len - 1)];
        while let Some((chunk, rest)) = bulk.split_first_chunk::<16>() {
            let x = u64::from_le_bytes(chunk[..8].try_into().unwrap());
            let y = u64::from_le_bytes(chunk[8..].try_into().unwrap());

            let t = multiply_mix(s0 ^ x, PREVENT_TRIVIAL_ZERO_COLLAPSE ^ y);
            s0 = s1;
            s1 = t;
            bulk = rest;
        }

        let suffix = &bytes[len - 16..];
        s0 ^= u64::from_le_bytes(suffix[0..8].try_into().unwrap());
        s1 ^= u64::from_le_bytes(suffix[8..16].try_into().unwrap());
    }

    multiply_mix(s0, s1) ^ (len as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::hash::{BuildHasher, Hash, Hasher};

    fn hash_bytes_raw(bytes: &[u8]) -> u64 {
        let mut hasher = FxHasher::new();
        hasher.write(bytes);
        hasher.finish()
    }

    #[test]
    fn test_fxhash_determinism_and_integers() {
        assert_eq!(FxBuildHasher.hash_one(0_u64), 0);
        assert_ne!(FxBuildHasher.hash_one(1_u64), 0);
        assert_ne!(FxBuildHasher.hash_one(1_u64), FxBuildHasher.hash_one(2_u64));
        assert_ne!(
            FxBuildHasher.hash_one(u32::MAX),
            FxBuildHasher.hash_one(u32::MAX - 1)
        );
        assert_ne!(
            FxBuildHasher.hash_one(1_u128),
            FxBuildHasher.hash_one(1_u128 << 64)
        );
    }

    #[test]
    fn test_fxhash_all_length_tiers_and_boundaries() {
        // Test every boundary transition in hash_bytes:
        // 0, 1..=3, 4..=7, 8..=16, 17..32 (16-byte chunks), exact multiples (16, 32, 48).
        let lengths = [
            0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 24, 31, 32, 33, 48, 64,
        ];
        let mut hashes = Vec::with_capacity(lengths.len());
        for &len in &lengths {
            let buf: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(31)).collect();
            let h = hash_bytes_raw(&buf);
            assert_eq!(
                h,
                hash_bytes_raw(&buf),
                "non-deterministic hash for len {len}"
            );
            assert!(!hashes.contains(&h), "unexpected collision at len {len}");
            hashes.push(h);
        }
    }

    #[test]
    fn test_fxhash_zero_inputs_do_not_collapse() {
        // Zeroed byte slices of distinct lengths must stay distinct.
        let z0 = hash_bytes_raw(&[]);
        let z1 = hash_bytes_raw(&[0]);
        let z8 = hash_bytes_raw(&[0; 8]);
        let z16 = hash_bytes_raw(&[0; 16]);
        let z32 = hash_bytes_raw(&[0; 32]);
        let set = [z0, z1, z8, z16, z32];
        for i in 0..set.len() {
            for j in (i + 1)..set.len() {
                assert_ne!(set[i], set[j], "zero collapse between index {i} and {j}");
            }
        }
    }

    #[test]
    fn test_fxhash_single_bit_and_position_sensitivity() {
        for len in [1_usize, 4, 8, 12, 16, 24, 40] {
            let base = vec![0x5Au8; len];
            let base_hash = hash_bytes_raw(&base);
            for pos in 0..len {
                let mut mutated = base.clone();
                mutated[pos] ^= 0x01;
                assert_ne!(
                    base_hash,
                    hash_bytes_raw(&mutated),
                    "single-bit flip at pos {pos} (len {len}) collided"
                );
            }
        }
    }

    #[test]
    fn test_fxhash_with_seed_differs() {
        let mut a = FxHasher::with_seed(1);
        let mut b = FxHasher::with_seed(2);
        "hello".hash(&mut a);
        "hello".hash(&mut b);
        assert_ne!(a.finish(), b.finish());
    }
}
