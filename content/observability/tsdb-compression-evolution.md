---
title: "The Evolution of TSDB Compression: From Gorilla to InfluxDB 3"
date: 2026-02-04
tags:
  - observability
  - tsdb
  - engineering
  - compression
---

# The Evolution of TSDB Compression: From Gorilla to InfluxDB 3

In the world of high-scale observability, data volume is the enemy of performance. A system collecting 10 million metrics per second generates nearly a trillion data points a day. Without aggressive compression, storage costs and query latency would make such systems economically and technically unfeasible.

Over the last decade, TSDB (Time Series Database) compression has evolved from custom-tailored bit-stream algorithms to standardized columnar formats. This article explores that journey, focusing on the techniques that defined an era.

## The Gorilla Era: Exploiting Regularity

In 2015, Facebook published the seminal paper **"Gorilla: A Fast, Scalable, In-Memory Time Series Database"**. It introduced two key techniques that became the industry standard for nearly a decade.

### 1. Timestamps: Delta-of-Delta Encoding

Time series data is usually periodic (e.g., every 10 seconds). In a perfect world, the difference between consecutive timestamps (the "delta") would be constant.

Instead of storing the timestamp ($t_n$), Gorilla stores the **Delta-of-Delta** ($D$):
$$D = (t_n - t_{n-1}) - (t_{n-1} - t_{n-2})$$

- If $D = 0$, Gorilla stores a single '0' bit.
- If $D$ is within a small range, it uses a few bits to store the value.
- Larger $D$ values use more bits with a specific prefix.

For perfectly regular metrics, this reduces the timestamp storage to a mere **1 bit per point**.

### 2. Values: XOR Compression

Floating-point values (IEEE 754) often change slowly. When you XOR two consecutive floating-point values ($v_n \oplus v_{n-1}$), many of the bits are identical, resulting in leading and trailing zeros.

- If the XOR sum is 0 (identical values), store a '0' bit.
- If not, store a '1' bit, followed by bits indicating the number of leading/trailing zeros, and then the "meaningful" bits of the XOR sum.

This approach allows Gorilla to compress typical time-series floats to an average of **1.37 bytes per point**.

## The Era of Specialization: Prometheus and TSM

Following Gorilla's success, systems like **Prometheus** adopted these techniques for their local storage (V2/V3). **InfluxDB**'s TSM (Time-Structured Merge Tree) engine took it further by adding:

- **Simple8b**: A bit-packing algorithm for integers.
- **RLE (Run-Length Encoding)**: For repeating values.
- **Bit-packing**: For booleans.

While highly efficient, these formats were "black boxes." If you wanted to analyze the data with Spark or DuckDB, you had to export it via an API, creating a bottleneck.

## The Paradigm Shift: InfluxDB 3 and Parquet

InfluxDB 3 (and the underlying IOx engine) represents a fundamental shift. It moved away from custom bit-stream formats toward **Apache Parquet**, a columnar storage standard.

### Why Parquet?

1. **Interoperability**: Parquet files can be read directly by almost any data engineering tool.
2. **Columnar Projection**: Only the columns needed for a query are read from disk.
3. **Advanced Compression**: Parquet supports modern algorithms like Zstandard and specialized encoders.

### DELTA_BINARY_PACKED

For timestamps and integers, InfluxDB 3 leverages Parquet’s `DELTA_BINARY_PACKED` encoding. This is the columnar cousin of Delta-of-delta.

Instead of bit-by-bit stream processing, it:
1. Calculates deltas for a block of values.
2. Finds the minimum delta in the block.
3. Subtracts that minimum from all deltas.
4. Uses **bit-packing** to store the results using the minimum number of bits required for the largest value in that block.

This is highly optimized for modern CPUs that prefer processing blocks of data (SIMD) rather than bit-at-a-time branching logic found in the original Gorilla implementation.

## Conclusion

The evolution from Gorilla to InfluxDB 3 is a story of maturing infrastructure. We have moved from specialized, "hand-crafted" compression aimed at saving every possible bit in RAM, to standardized, block-oriented columnar formats optimized for disk I/O and ecosystem interoperability.

As we move into the era of "Observability Data Lakes," the ability to compress data without locking it away in a proprietary format has become the new gold standard.

---
**References:**
- [Gorilla: A Fast, Scalable, In-Memory Time Series Database (2015)](https://www.vldb.org/pvldb/vol8/p1816-teller.pdf)
- [InfluxDB 3.0 Documentation: Storage Engine](https://docs.influxdata.com/influxdb/v3/)
- [Apache Parquet Encoding Specifications](https://parquet.apache.org/docs/file-format/data-pages/encodings/)
