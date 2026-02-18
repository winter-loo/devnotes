---
title: "Probabilistic Counting: The Bit-Level Internals of HyperLogLog"
date: 2026-02-18
tags:
  - observability
  - algorithms
  - rust
  - redis
  - performance
---

# Probabilistic Counting: The Bit-Level Internals of HyperLogLog

In distributed systems, answering "How many unique users visited today?" is surprisingly expensive. A naïve approach using a `HashSet` or `BTreeSet` scales linearly in memory ($O(N)$). For 100 million users, storing 64-bit user IDs requires ~800MB.

**HyperLogLog (HLL)** solves this with a constant memory footprint (typically 12KB) and a standard error of ~0.81%. It achieves this not by counting, but by **observing probability**.

This article dissects the bit-level mechanics of HLL, implements the core register update logic in Rust, and explores the sparse-vs-dense architectural trade-offs found in Redis.

## The Core Intuition: Coin Flips

If you flip a coin and get `Heads`, it's unremarkable (50% chance).
If you get `Tails, Heads`, it's 25%.
If you get `Tails, Tails, Tails, Heads` ($k=3$), it's 6.25% ($1/2^{k+1}$).

If someone tells you their longest run of trailing tails was $k=10$, you can estimate they flipped the coin roughly $2^{10} \approx 1024$ times. HLL applies this to hashes.

## Bit-Level Implementation

HLL uses a hash function (like MurmurHash64 or xxHash) to turn arbitrary data into a uniform 64-bit integer. This integer is split into two parts:

1.  **Bucket Index ($p$ bits)**: Determines which "register" to update.
2.  **Run of Zeros ($w$ bits)**: The "coin flip" sequence.

For a standard HLL configuration ($m=16384$ registers), we use the first $p=14$ bits for the index.

### The Layout

```mermaid
graph LR
    subgraph Hash_64_Bit [64-bit Hash Value]
    direction LR
    Bucket[Bucket Index<br/>p=14 bits] --- W[Remaining Bits<br/>w=50 bits]
    end

    Bucket --> |Selects| Reg[Register M[idx]]
    W --> |Count Leading Zeros| Rho[ρ(w)]
    Rho --> |Update| Max{Max(M[idx], ρ)}
    Max --> Reg
```

**Register Width:**
Since the maximum run of zeros in the remaining 50 bits is 50, we only need 6 bits ($2^6 = 64$) to store the count. Thus, the total memory is:
$$ M = 2^{14} \text{ registers} \times 6 \text{ bits} = 98,304 \text{ bits} = 12 \text{ KB} $$

### Rust Implementation: The Register Update

The core operation is the `update` step. It must be extremely fast as it happens for every element.

```rust
pub struct HyperLogLog {
    registers: [u8; 16384], // 16384 registers (using u8 for simplicity, though 6 bits is optimal)
    p: u8,
}

impl HyperLogLog {
    /// Adds an element to the HLL sketch
    /// 
    /// # Bit-Level Logic
    /// 1. Hash the element to u64.
    /// 2. Extract top p bits for bucket index.
    /// 3. Count leading zeros of the remaining (64-p) bits.
    /// 4. Update register if new count > old count.
    pub fn add(&mut self, hash: u64) {
        // Extract the bucket index (first p bits)
        // We shift right by (64 - p) to move the top p bits to the LSB
        let idx = (hash >> (64 - self.p)) as usize;

        // Mask out the bucket bits to leave the "w" bits
        // We construct a mask of (1 << (64 - p)) - 1
        // But simpler: just shift left p bits to clear top, then trailing zeros handles it
        // Note: The paper defines rho(w) as position of 1st '1' bit (1-indexed)
        // This is equivalent to leading_zeros() + 1 on the remaining portion
        
        let w = hash << self.p; // Shift out the index bits
        
        // Count leading zeros of the remaining 64-bit word. 
        // If w is 0, we treat it as max run (though technically impossible with perfect hash)
        let zeros = w.leading_zeros() as u8;
        let rho = zeros + 1;

        // The "Max" operation - branchless in assembly (CMOV)
        if rho > self.registers[idx] {
            self.registers[idx] = rho;
        }
    }
}
```

## SIMD Merging: The Hidden Power

A major advantage of HLL is that sketches are **mergeable**. The union of two HLLs is simply the element-wise maximum of their registers:
$$ M_{union}[i] = \max(M_A[i], M_B[i]) $$

This is a perfect candidate for SIMD. We can process 32 or 64 registers in a single instruction cycle using AVX-512 or NEON.

```rust
use std::simd::{u8x64, SimdOrd}; // Nightly Rust feature

pub fn merge_registers_simd(dest: &mut [u8; 16384], src: &[u8; 16384]) {
    let chunks_dest = dest.as_chunks_mut::<64>().0;
    let chunks_src = src.as_chunks::<64>().0;

    for (d, s) in chunks_dest.iter_mut().zip(chunks_src.iter()) {
        let v_dest = u8x64::from_array(*d);
        let v_src = u8x64::from_array(*s);
        
        // Single instruction: VPMAXUB (AVX-512)
        // Computes max of each byte lane in parallel
        let v_max = v_dest.simd_max(v_src);
        
        *d = v_max.to_array();
    }
}
```
**Architectural Note:** This merge capability is why distributed databases (like Druid or Pinot) store HLL sketches directly. You can query cardinality across 100 shards by merging 100 12KB blobs (Total ~1.2MB) on the aggregator, which takes microseconds with SIMD.

## The Bias Correction (Harmonic Mean)

You cannot just average the registers. One "lucky" run of 50 zeros would skew the arithmetic mean massively. HLL uses the **Harmonic Mean** to dampen the effect of outliers.

$$ E = \alpha_m m^2 \left( \sum_{j=1}^{m} 2^{-M[j]} \right)^{-1} $$

Where $\alpha_m$ is a pre-calculated bias correction constant (0.7213 / (1 + 1.079/m)).

Even then, for small cardinalities (where many registers are 0), HLL is inaccurate. Google's **HLL++** algorithm and Redis introduce "LinearCounting" (counting empty registers) to correct estimates when $E < \frac{5}{2}m$.

## The Sparse vs. Dense Paradox

Redis's implementation ([`pfcount`](https://redis.io/commands/pfcount/)) uses a clever optimization: **Sparse Representation**.

For low cardinalities, storing 12KB of mostly `0`s is wasteful. Redis initially stores the registers as a compressed list of opcodes:
- `ZERO:len` (Run length encoding of zeros)
- `VAL:value` (Actual register value)
- `XZERO:len` (Extended run of zeros)

**The Trade-off:**
1.  **Memory:** Sparse is tiny (< 100 bytes for small sets).
2.  **CPU:** Updating a sparse list requires shifting memory (like `Vec::insert`). It is $O(N)$ relative to the encoded length.
3.  **Transition:** Once the encoded size exceeds a threshold (usually 3KB) or a register value exceeds 32, Redis converts it to the standard **Dense** (12KB fixed) representation.

**Research Question:**
Why does Redis cap the sparse representation at 3KB?
Modern CPU caches (L1) are 32KB-48KB. A 12KB dense HLL fits entirely in L1. The cost of decoding the sparse format (branching, parsing opcodes) often exceeds the cost of just blasting through a 12KB array, especially given the memory bandwidth of modern servers.

Is the "Sparse" optimization a relic of an era where RAM was more expensive than CPU cycles? For high-throughput streams, **always-dense** might actually be faster due to branch prediction stability, despite the memory penalty.

## Conclusion

HLL is a masterpiece of bit-level engineering. It trades accuracy for memory, but does so with a layout that is cache-friendly and SIMD-accelerated.

However, for exact counting at low cardinalities, **Roaring Bitmaps** (which switch between arrays of integers and bitsets) often provide better utility (exact counts + set operations) at a comparable memory cost until the set becomes very dense.

---
**Technical References:**
- [HyperLogLog: The Analysis of a Near-Optimal Cardinality Estimation Algorithm](http://algo.inria.fr/flajolet/Publications/FlFuGaMe07.pdf) (Flajolet et al.)
- [Redis HLL Source Code (hyperloglog.c)](https://github.com/redis/redis/blob/unstable/src/hyperloglog.c)
- [Google HLL++ Algorithm](https://static.googleusercontent.com/media/research.google.com/en//pubs/archive/40671.pdf)
- [Rust `std::simd` Documentation](https://doc.rust-lang.org/std/simd/index.html)
