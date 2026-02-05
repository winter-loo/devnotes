---
title: "The Evolution of TSDB Compression: From Gorilla to InfluxDB 3"
date: 2026-02-05
tags:
  - observability
  - tsdb
  - engineering
  - compression
  - rust
---

# The Evolution of TSDB Compression: From Gorilla to InfluxDB 3

In the world of high-scale observability, data volume is the enemy of performance. A system collecting 10 million metrics per second generates nearly a trillion data points a day. Without aggressive compression, storage costs and query latency would make such systems economically and technically unfeasible.

Over the last decade, TSDB (Time Series Database) compression has evolved from custom-tailored bit-stream algorithms to standardized columnar formats. This article explores that journey, focusing on the techniques that defined an era.

## The Gorilla Era: Exploiting Regularity

In 2015, Facebook published the seminal paper [**"Gorilla: A Fast, Scalable, In-Memory Time Series Database"**](https://www.vldb.org/pvldb/vol8/p1816-teller.pdf). It introduced two key techniques that became the industry standard for nearly a decade: Delta-of-Delta for timestamps and XOR compression for values.

### 1. Timestamps: Delta-of-Delta Encoding

Most time-series data is periodic (e.g., every 60 seconds). In a perfect world, the difference between consecutive timestamps (the "delta") is constant. Delta-of-Delta encoding exploits this regularity.

#### Step-by-Step Walk-through
Let's compress the sequence: `[1643673600, 1643673660, 1643673722, 1643673780]`

| Timestamp | Value ($t_n$) | Delta ($\Delta$) | Delta-of-Delta ($D$) | Bits Stored (Gorilla) |
| :--- | :--- | :--- | :--- | :--- |
| $t_0$ | 1643673600 | - | - | Full 64 bits |
| $t_1$ | 1643673660 | 60 | - | Full 14-bit Delta |
| $t_2$ | 1643673722 | 62 | $62 - 60 = 2$ | `10` + `0000010` (9 bits) |
| $t_3$ | 1643673780 | 58 | $58 - 62 = -4$ | `10` + `1111100` (9 bits) |

**Bitstream State:**
To decode $t_2$, the reader must have already decoded $t_0$, $t_1$, and the first delta ($60$). The bitstream is fundamentally **serial**: you cannot calculate $t_n$ without traversing all previous bits to maintain the running sum of deltas.

### 2. Values: XOR Compression

Floating-point values ([IEEE 754](https://en.wikipedia.org/wiki/IEEE_754)) often change slowly. When you XOR two consecutive values ($v_n \oplus v_{n-1}$), identical bits result in `0`.

```mermaid
bitfield
  0-1: "Control (00/10/11)"
  2-6: "Leading Zeros (5 bits)"
  7-12: "Meaningful Length (6 bits)"
  13-40: "XORed Data (Variable)"
  41-63: "Trailing Zeros (Implicit)"
```

*Note: The diagram above represents the logical structure of a compressed XOR chunk. If the XOR result is the same as the previous, we store only a single `0` bit.*

## Implementation: The Core XOR Logic

While trivial functions like `new()` are common, the "magic" of Gorilla happens in the bit-level masking during XOR encoding.

```rust
fn encode_xor(&mut self, xor: u64, leading: u32, trailing: u32) {
    if xor == 0 {
        self.writer.write_bit(false); // Store '0'
    } else {
        self.writer.write_bit(true); // Store '1'
        
        if leading >= self.last_leading && trailing >= self.last_trailing {
            // Control '0': Use previous leading/trailing counts
            self.writer.write_bit(false);
            let meaningful_bits = 64 - self.last_leading - self.last_trailing;
            self.writer.write_bits(xor >> self.last_trailing, meaningful_bits);
        } else {
            // Control '1': New leading/trailing counts
            self.writer.write_bit(true);
            self.writer.write_bits(leading as u64, 5); // 5 bits for leading (0-31+)
            
            let meaningful_bits = 64 - leading - trailing;
            self.writer.write_bits(meaningful_bits as u64, 6); // 6 bits for length
            self.writer.write_bits(xor >> trailing, meaningful_bits);
            
            self.last_leading = leading;
            self.last_trailing = trailing;
        }
    }
}
```

## The Paradigm Shift: Serial vs. SIMD

The primary limitation of Gorilla is its **"bit-at-a-time branching"**. Because every value's interpretation depends on the control bits of the previous value, the CPU's branch predictor is heavily taxed, and instruction-level parallelism (ILP) is limited.

In contrast, modern engines like InfluxDB 3 (via Apache Parquet) utilize **SIMD block processing**.

| Aspect | Gorilla (Bitstream) | Parquet (SIMD/Bit-packing) |
| :--- | :--- | :--- |
| **Data Layout** | Hybrid (Values interleaved) | Pure Columnar |
| **Processing** | Serial (Bit-by-bit) | Vectorized (128+ values at once) |
| **Hardware** | Scalar CPU | AVX-512 / NEON |
| **Random Access** | Impossible (must scan) | Possible at block boundaries |

## The VictoriaMetrics Paradox: Why Go still beats Rust/SIMD?

Despite the theoretical superiority of SIMD and Rust's low-level control, [VictoriaMetrics](https://victoriametrics.com/) (written in Go) consistently outperforms many "modern" Rust TSDBs. This raises a provocative question: **Is the architectural overhead of Parquet worth it?**

1.  **The "Go SIMD" Reality**: Go lacks direct SIMD intrinsics in the language, yet VM achieves incredible throughput by using highly optimized assembly for critical loops and sticking to a "shared-nothing" architecture that minimizes GC pressure.
2.  **Parquet's Tax**: While Parquet is great for interoperability, the overhead of its complex metadata and the "shredding" required to turn rows into columns can outweigh the raw speed of SIMD if the query pattern is simple.
3.  **The Bitstream Defense**: VictoriaMetrics proves that if you optimize the *memory access patterns* and minimize allocations, a well-tuned bitstream implementation can often beat a generic columnar implementation due to better cache locality for specific time-series workloads.

The trade-off remains: Do you want the **ecosystem compatibility** of Parquet/Arrow, or the **raw, specialized efficiency** of a custom bitstream?

---
**Technical References:**
- [IEEE 754 Standard for Floating-Point Arithmetic](https://ieeexplore.ieee.org/document/8766229)
- [Gorilla: Facebook's In-Memory TSDB](https://www.vldb.org/pvldb/vol8/p1816-teller.pdf)
- [Bit-packing Explained (Lemire)](https://github.com/lemire/FastPFor) - *High-performance integer compression techniques.*
- [Apache Parquet Encoding Specifications](https://parquet.apache.org/docs/file-format/data-pages/encodings/)
- [Rust `std::simd` Documentation](https://doc.rust-lang.org/std/simd/index.html)
