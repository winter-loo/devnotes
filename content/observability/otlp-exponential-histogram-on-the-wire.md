---
title: "OTLP Exponential Histograms on the Wire: Bucket Index Math, Packed Varints, and Why Your Collector CPU Spikes"
date: 2026-03-11
tags:
  - observability
  - opentelemetry
  - otlp
  - protobuf
  - rust
  - histograms
---

# OTLP Exponential Histograms on the Wire: Bucket Index Math, Packed Varints, and Why Your Collector CPU Spikes

If you’ve ever enabled OpenTelemetry metrics, switched a latency metric from classic buckets to an **ExponentialHistogram**, and watched your collector suddenly spend real CPU in “encoding”, you’ve met the thing this post is about:

- Exponential histograms are *algorithmically* elegant,
- but their **wire encoding is aggressively varint-heavy**,
- and the naive implementation is a branchy mess (float → log → integer → zigzag → varint → packed repeated field).

This is a deep dive into the exact bytes you ship when you export an OTLP metric containing an `ExponentialHistogramDataPoint`.

We’ll do three things:

1. Nail the **bucket index math** (which exact interval does a value fall into?)
2. Show the **protobuf wire records** you actually emit (tag varints, zigzag, packed repeated)
3. Write **Rust** that encodes the hot path without allocations and with predictable branches

## Specs we’re going to rely on (verified)

These links were manually fetched/checked while writing:

- Protobuf wire format (varints, tags, LEN records, packed repeated):
  - https://protobuf.dev/programming-guides/encoding/

- OTLP Metrics `ExponentialHistogramDataPoint` definition (scale/base formula, `Buckets.offset`, `Buckets.bucket_counts` = `repeated uint64`):
  - https://raw.githubusercontent.com/open-telemetry/opentelemetry-proto/main/opentelemetry/proto/metrics/v1/metrics.proto

- OTLP MetricsService request wrapper (`ExportMetricsServiceRequest.resource_metrics = 1`):
  - https://raw.githubusercontent.com/open-telemetry/opentelemetry-proto/main/opentelemetry/proto/collector/metrics/v1/metrics_service.proto

- Prometheus histogram type refresher (classic `_bucket{le=...}` vs “native histograms” note):
  - https://prometheus.io/docs/concepts/metric_types/#histogram

## The data structure we’re encoding (what fields are hot)

From `metrics.proto`, the key fields are:

- `sint32 scale = 6;`
- `fixed64 zero_count = 7;`
- `Buckets positive = 8;` / `Buckets negative = 9;`

And `Buckets` is:

- `sint32 offset = 1;`
- `repeated uint64 bucket_counts = 2;`

Two details matter for performance and bytes:

1. `sint32` uses **ZigZag** encoding (so `-1` becomes `1`, etc.) and then a varint.
2. `repeated uint64` in proto3 is **packed by default**, so the wire shape is:

   - tag (field 2, wire type LEN)
   - length (varint)
   - concatenated varints of each `bucket_counts[i]`

That “concatenated varints” part is why sparse buckets (lots of zeros) can be cheap *if you don’t transmit trailing zeros* — and why dense buckets can still hurt.

## Bucket index math (precise interval semantics)

The proto comment defines:

- `base = 2^(2^-scale)`
- bucket `index` contains values:

  \( (base^{index},\; base^{index+1}] \)

So for positive values, the index function is:

\[
index(v) = \left\lfloor \log_{base}(v) \right\rfloor
= \left\lfloor \frac{\log_2(v)}{\log_2(base)} \right\rfloor
= \left\lfloor \log_2(v) \cdot 2^{scale} \right\rfloor
\]

Because \(\log_2(base) = 2^{-scale}\).

That last form is the one you want in an implementation: **a single `log2` times a power-of-two**.

### Worked example (scale = 3)

For `scale = 3`:

- `base = 2^(2^-3) = 2^(1/8) ≈ 1.090507732`
- `index(v) = floor(log2(v) * 8)`

Now pick a few values:

| v | log2(v) | log2(v)*8 | index(v) | bucket interval (approx) |
|---:|---:|---:|---:|---|
| 1.0 | 0.0000 | 0.0000 | 0 | (1.0000, 1.0905] |
| 1.09 | 0.1250 | 1.0000 | 1 | (1.0905, 1.1892] |
| 2.0 | 1.0000 | 8.0000 | 8 | (2.0000, 2.1810] |
| 10.0 | 3.3219 | 26.575 | 26 | (≈9.513, ≈10.358] |

Two sharp edges:

- The interval is **open on the lower bound, closed on the upper bound**.
- `v = base^k` is *not* in bucket `k` — it belongs to bucket `k-1` because the lower bound is open.

In practice, SDKs typically incorporate a tiny epsilon or rely on IEEE rounding; as long as encoder+decoder agree, you’re fine — but if you implement your own, test boundary values.

## Offsets + contiguous arrays: the compression model

The `Buckets` message compresses an infinite sparse map `{index -> count}` into:

- `offset`: first index present
- `bucket_counts[i]`: count at index `offset + i`

So if your non-empty indices are `{26, 27, 30}` you either:

- pay for holes by including zeros between them, or
- split into multiple histograms (you can’t; the message only has one contiguous run per sign)

Which implies a hidden cost trade-off:

- Higher `scale` ⇒ more indices in-range ⇒ more “holes” unless your distribution is tight.

### Visual: mapping index space to the packed array

```mermaid
flowchart TB
  subgraph IndexSpace[Bucket index space]
    I24((24)) --- I25((25)) --- I26((26)) --- I27((27)) --- I28((28)) --- I29((29)) --- I30((30))
  end

  subgraph BucketsMessage[OTLP Buckets message]
    Off[offset = 26]
    Arr[bucket_counts = [c0,c1,c2,c3,c4]]
  end

  I26 -->|c0| Arr
  I27 -->|c1| Arr
  I28 -->|c2 (maybe 0)| Arr
  I29 -->|c3 (maybe 0)| Arr
  I30 -->|c4| Arr
```

This is why exporters try hard to **trim leading/trailing zeros** and to keep scale reasonable.

## Protobuf wire format: bytes, not vibes

Protobuf encodes a message as a sequence of *records*:

- tag = `(field_number << 3) | wire_type` encoded as a **varint**
- payload depends on wire type

We care about three wire types here:

- VARINT (wire type 0) → varint payload (used for `sint32` after ZigZag and for `uint64` counts)
- I64 (wire type 1) → fixed 8 bytes little-endian (used for `fixed64 count`, `fixed64 zero_count`)
- LEN (wire type 2) → `len(varint) + bytes` (used for submessages + packed repeated)

### Concrete mini-message we’ll encode

Let’s encode *just* a `Buckets` submessage for the positive side:

- `offset = 26`
- `bucket_counts = [3, 0, 12]`

`offset` is `sint32` (ZigZag), `bucket_counts` are `uint64` (varint), packed.

#### Step 1: encode `offset = 26`

- ZigZag(26) = 52
- tag for field 1 (offset), VARINT: `(1<<3)|0 = 8` → `0x08`
- varint(52) = `0x34`

So bytes:

| bytes | meaning |
|---|---|
| `08` | tag: field 1, VARINT |
| `34` | ZigZag(offset)=52 |

#### Step 2: encode packed `bucket_counts = [3,0,12]`

Packed repeated is a LEN record:

- tag for field 2, LEN: `(2<<3)|2 = 18` → `0x12`
- payload bytes are varint(3) + varint(0) + varint(12) = `03 00 0c`
- length = 3 → `0x03`

So bytes:

| bytes | meaning |
|---|---|
| `12` | tag: field 2, LEN |
| `03` | length of packed payload |
| `03 00 0c` | concatenated varints |

#### Final `Buckets` submessage bytes

Concatenate:

```
08 34 12 03 03 00 0c
```

That is the *inner* bytes. When you embed `Buckets positive = 8;` inside `ExponentialHistogramDataPoint`, those bytes themselves become a LEN payload with their own outer tag and length.

## Rust: encode the hot path (varints + packed repeated) without allocations

The goal is not “use `prost`” (you should). The goal is to understand and control the hot path when:

- you’re writing a collector/receiver,
- you’re doing custom aggregation,
- or you want to pre-encode parts of a message.

Below is a minimal encoder for:

- unsigned varints (`u64`)
- ZigZag for `sint32`
- protobuf tag
- packed repeated `uint64`

It’s intentionally pointer-y.

```rust
#[inline]
fn put_varint_u64(mut x: u64, out: &mut Vec<u8>) {
    // Worst-case 10 bytes.
    // Fast path: write into a small stack buffer then extend.
    let mut buf = [0u8; 10];
    let mut i = 0usize;

    while x >= 0x80 {
        buf[i] = (x as u8 & 0x7f) | 0x80;
        x >>= 7;
        i += 1;
    }
    buf[i] = x as u8;
    i += 1;

    out.extend_from_slice(&buf[..i]);
}

#[inline]
fn zigzag_i32(x: i32) -> u32 {
    // (n << 1) ^ (n >> 31)
    ((x << 1) ^ (x >> 31)) as u32
}

#[inline]
fn put_tag(field_number: u32, wire_type: u32, out: &mut Vec<u8>) {
    let tag = ((field_number << 3) | wire_type) as u64;
    put_varint_u64(tag, out);
}

/// Encode the Buckets message:
/// message Buckets { sint32 offset = 1; repeated uint64 bucket_counts = 2; }
fn encode_buckets(offset: i32, counts: &[u64], out: &mut Vec<u8>) {
    // field 1: offset (VARINT)
    put_tag(1, 0, out);
    put_varint_u64(zigzag_i32(offset) as u64, out);

    // field 2: bucket_counts (packed, so LEN)
    put_tag(2, 2, out);

    // Precompute packed payload length in bytes (sum varint sizes)
    let mut payload_len = 0usize;
    for &c in counts {
        payload_len += varint_len_u64(c);
    }

    put_varint_u64(payload_len as u64, out);
    for &c in counts {
        put_varint_u64(c, out);
    }
}

#[inline]
fn varint_len_u64(mut x: u64) -> usize {
    let mut n = 1usize;
    while x >= 0x80 {
        x >>= 7;
        n += 1;
    }
    n
}
```

### Where the CPU goes (and what to do about it)

The exporter/collector cost is typically dominated by:

1. **Computing indices**: `log2(v)` (expensive) + multiplication by `2^scale`
2. **Varint encoding** of many `bucket_counts` values
3. **Branch mispredicts** in (a) bucket trimming logic and (b) varint loops

Common mitigations:

- Keep scale moderate. Higher scale increases both the number of buckets and the likelihood of holes.
- Trim zeros aggressively, but do it in a cache-friendly way (scan from both ends once).
- Consider approximating `log2` for latency-like metrics where perfect boundary fidelity isn’t required.

## Architecture trade-offs: ExponentialHistogram vs classic Prometheus buckets vs “just store raw”

### ExponentialHistogram (OTel)

Pros:

- Constant-relative-error bucket widths: a good fit for latency and sizes.
- Can represent very wide dynamic ranges without predefining explicit bounds.
- Wire encoding uses varints; zeros are cheap *if you avoid sending them*.

Cons:

- Index math is float-heavy (`log2`).
- Offset+contiguous array means holes cost you zeros.
- Decoding/merging histograms is more complex than summing counters.

### Classic Prometheus histogram buckets

Pros:

- Bucket boundaries are explicit and human-controlled.
- Scrape model naturally amortizes encoding: it’s plaintext, and storage is the expensive part anyway.

Cons:

- Many time series per metric (`_bucket{le=...}`, `_sum`, `_count`).
- High resolution = high cardinality = expensive ingestion.

### Storage formats: Parquet vs custom TSDB blocks (the “observability warehouse” argument)

If your endgame is long-retention analytics:

- **Parquet** can compress well with columnar encodings (RLE/bit-pack/delta), but it prefers *append-only batch* and stable schemas.
- Custom TSDB blocks (Prometheus-style XOR for floats, varint+delta for ints, or TSM-like encodings) are optimized for *time-locality* and streaming writes.

Exponential histograms are awkward in Parquet unless you normalize them into a fixed schema (which defeats their dynamic nature), while TSDB blocks can store `(offset, counts[])` efficiently if you treat counts as a short varint-packed blob.

Language note:

- A Go exporter often wins here not because Go is “faster”, but because the standard libraries and allocators make the “many small appends” workload less pathological than naive Rust `Vec` growth patterns.
- Rust can win hard once you pre-size buffers and avoid iterator overhead, but you have to *write the ugly code*.

## Research question (a paradox to end on)

If varint-heavy packed fields are the whole point (cheap zeros, cheap small integers), why do real-world collectors often get slower as you increase histogram resolution *even when the number of non-zero buckets stays roughly constant*?

Is the bottleneck:

- `log2` throughput,
- branch mispredicts in trimming/encoding,
- allocator/cache behavior,
- or an emergent property of merging many small packed blobs?

If you can build a benchmark where a Go implementation outperforms a carefully pre-sized, branch-tamed Rust encoder for the same ExponentialHistogram stream… what does that imply about where we should be investing optimization effort: math, encoding, or memory systems?
