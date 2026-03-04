---
title: "Prometheus Remote-Write on the Wire: Snappy Block + Protobuf Varints (and Why Your Ingest Costs Money)"
date: 2026-03-04
tags:
  - observability
  - prometheus
  - protobuf
  - snappy
  - rust
---

# Prometheus Remote-Write on the Wire: Snappy Block + Protobuf Varints (and Why Your Ingest Costs Money)

Remote-Write is one of those protocols that *feels* simple (“POST some samples”), until you try to:

- debug ingestion corruption across languages,
- implement a fast receiver (Rust, C++, Java) that doesn’t allocate itself to death,
- or explain why a small change in label cardinality doubled your bill.

This deep dive is about the **exact bytes on the wire**:

1. HTTP request headers (what must be present)
2. **Snappy block** encoding (not framed)
3. **Protocol Buffers** binary wire format (varints + length-delimited submessages)

…and how each layer turns “a few metrics” into CPU, memory bandwidth, and storage amplification.

## Specs we’re going to rely on (verified)

- Prometheus Remote-Write 1.0 spec (HTTP headers, Snappy *block* format requirement):
  - https://prometheus.io/docs/specs/prw/remote_write_spec/
- Prometheus Remote-Write 2.0 spec (content-type negotiation, still Snappy block):
  - https://prometheus.io/docs/specs/prw/remote_write_spec_2_0/
- Protobuf wire format (varints, tags, wire types):
  - https://protobuf.dev/programming-guides/encoding/
- Snappy *block* format description (preamble varint length + tag byte low 2 bits):
  - https://raw.githubusercontent.com/google/snappy/2c94e11145f0b7b184b831577c93e5a41c4c0346/format_description.txt

## What the HTTP request actually is

Remote-Write 1.0 mandates:

- HTTP POST body is **Snappy-compressed protobuf**
- Headers MUST include:
  - `Content-Encoding: snappy`
  - `Content-Type: application/x-protobuf`
  - `X-Prometheus-Remote-Write-Version: 0.1.0`

The spec also explicitly says:

- Snappy **block format MUST be used**
- Snappy **framed format MUST NOT be used**

(Source: Remote-Write 1.0 spec, “Protocol” section: https://prometheus.io/docs/specs/prw/remote_write_spec/)

Remote-Write 2.0 keeps the same idea, but formalizes schema negotiation via `Content-Type: application/x-protobuf;proto=...` and response “written” counters.

## Layer 1: Snappy block format (byte- and bit-level)

A Snappy *block* stream is:

1. **Uncompressed length** as a *little-endian varint*
2. Then a sequence of **elements**, each starting with a **tag byte**

From the Snappy format description (preamble + element types):
https://raw.githubusercontent.com/google/snappy/2c94e11145f0b7b184b831577c93e5a41c4c0346/format_description.txt

### Snappy’s tag byte: the low 2 bits decide everything

The low two bits (`tag & 0b11`) select the element type:

| `tag & 0b11` | Meaning | Payload that follows |
|---:|---|---|
| `0b00` | Literal | literal bytes (length derived from high bits) |
| `0b01` | Copy (1-byte offset) | 1 extra byte offset, length in tag bits |
| `0b10` | Copy (2-byte offset) | 2 bytes LE offset, length in tag bits |
| `0b11` | Copy (4-byte offset) | 4 bytes LE offset, length in tag bits |

This matters operationally: even “fast” Snappy decode is a **bytecode interpreter** (branchy loop) that often becomes a throughput limiter *before* protobuf parsing.

### Literal lengths (the `00` case)

For literals up to 60 bytes, length is in the upper 6 bits:

- `len = (tag >> 2) + 1`

For longer literals, the tag indicates how many bytes follow that contain `(len-1)` in little-endian.

### A mermaid diagram that matches the *real* byte layout

```mermaid
graph TD
  A[HTTP POST body bytes] --> B[Snappy block stream]
  B --> C[Varint: uncompressed_length]
  B --> D[Elements...]
  D --> E[Tag byte]
  E -->|00| F[Literal: N bytes]
  E -->|01/10/11| G[Copy: (offset,length)]
  F --> H[Produces protobuf bytes]
  G --> H
```

Not fancy—but it reflects the actual serialization nesting:
**HTTP → Snappy → Protobuf**.

## Layer 2: Protobuf wire format (varints + TLV records)

Protobuf messages are a sequence of *records*:

- **tag** = varint((field_number << 3) | wire_type)
- **payload** = depends on wire type

Wire types we care about here:

- `0` VARINT: `int64`, `bool`, enums
- `1` I64: fixed 8 bytes (e.g. `double`)
- `2` LEN: bytes/string/submessage/packed repeated

(Source: Protobuf encoding guide, “Message Structure” and “Base 128 Varints”: https://protobuf.dev/programming-guides/encoding/)

## The Remote-Write v1 protobuf: `prometheus.WriteRequest`

The canonical v1 `.proto` is in Prometheus itself (raw):

- https://raw.githubusercontent.com/prometheus/prometheus/main/prompb/remote.proto

It defines:

```proto
message WriteRequest {
  repeated prometheus.TimeSeries timeseries = 1;
  reserved 2;
  repeated prometheus.MetricMetadata metadata = 3;
}
```

(For v1 spec purposes, field 3 is reserved/omitted, but real Prometheus uses it.)

The important thing: **`repeated TimeSeries timeseries = 1`** means the write request is a sequence of field-1, length-delimited submessages.

## A fully worked byte-level example (with offsets)

We will encode a single `WriteRequest` containing one `TimeSeries`:

- Labels:
  - `__name__ = "cpu_usage"`
  - `instance = "a"`
- Samples:
  - `value = 1.5`
  - `timestamp = 1700000000000` (ms since epoch)

Below is the resulting **protobuf bytes** (hex):

```
0a 38
   0a 15 0a 08 5f5f6e616d655f5f 12 09 6370755f7573616765
   0a 0d 0a 08 696e7374616e6365 12 01 61
   12 10 09 000000000000f83f 10 80d095ffbc31
```

Where:

- `0a` is tag `(field=1, wire=LEN)` because `(1<<3)|2 = 10 = 0x0a`
- `38` is length of the embedded `TimeSeries` message: 56 bytes

### Offset table (selected bytes)

| Off | Hex | Meaning |
|---:|---|---|
| 0 | `0a` | WriteRequest field=1, LEN (timeseries) |
| 1 | `38` | length=56 |
| 2 | `0a` | TimeSeries field=1, LEN (labels) |
| 3 | `15` | length=21 |
| 4 | `0a` | Label field=1, LEN (name) |
| 5 | `08` | length=8 |
| 6..13 | `5f5f6e616d655f5f` | "__name__" |
| 14 | `12` | Label field=2, LEN (value) |
| 15 | `09` | length=9 |
| 16..24 | `6370755f7573616765` | "cpu_usage" |
| 25 | `0a` | TimeSeries field=1, LEN (labels) (second label) |
| 27 | `0a` | Label field=1, LEN (name) |
| 35 | `12` | Label field=2, LEN (value) |
| 38 | `12` | TimeSeries field=2, LEN (samples) |
| 39 | `10` | length=16 |
| 40 | `09` | Sample field=1, I64 (double value) |
| 41..48 | `000000000000f83f` | 1.5 as IEEE754 little-endian |
| 49 | `10` | Sample field=2, VARINT (timestamp) |
| 50..55 | `80d095ffbc31` | 1700000000000 as base-128 varint |

If your receiver ever “mysteriously” mis-parses timestamps, it is almost always:

- decoding varints as big-endian chunks,
- truncating to 32-bit,
- or accidentally interpreting `int64` as zigzag `sint64`.

## Rust: a Snappy *block* decoder with explicit bit operations

Most production systems should use a battle-tested library.
But if you’re writing a receiver and want to understand the CPU cost, you need to understand the loop.

Below is a correct, bounds-checked Snappy block decoder skeleton that implements:

- varint32 preamble length
- literal parsing (`00`)
- copy parsing (`01/10/11`)

It’s intentionally “crunchy”: lots of masking/shifting, and careful slice bounds.

```rust
#[derive(Debug)]
pub enum SnappyError {
    UnexpectedEof,
    BadOffset,
    OutputTooLarge,
    BadVarint,
}

fn read_uvarint32(input: &[u8], mut i: usize) -> Result<(u32, usize), SnappyError> {
    let mut x: u32 = 0;
    let mut s: u32 = 0;

    // Varint: little-endian base-128.
    while i < input.len() {
        let b = input[i] as u32;
        i += 1;
        if b < 0x80 {
            if s >= 32 && b > 0 {
                return Err(SnappyError::BadVarint);
            }
            return Ok((x | (b << s), i));
        }
        x |= (b & 0x7f) << s;
        s += 7;
        if s >= 35 {
            return Err(SnappyError::BadVarint);
        }
    }
    Err(SnappyError::UnexpectedEof)
}

pub fn decode_snappy_block(input: &[u8]) -> Result<Vec<u8>, SnappyError> {
    let (out_len, mut i) = read_uvarint32(input, 0)?;
    let out_len = out_len as usize;

    let mut out = Vec::with_capacity(out_len);

    while i < input.len() {
        let tag = input[i];
        i += 1;

        match tag & 0b11 {
            0b00 => {
                // Literal.
                let mut len = (tag >> 2) as usize;
                if len < 60 {
                    len += 1;
                } else {
                    let nbytes = (len - 59) as usize; // 1..4 bytes follow.
                    if i + nbytes > input.len() {
                        return Err(SnappyError::UnexpectedEof);
                    }
                    let mut v: usize = 0;
                    for k in 0..nbytes {
                        v |= (input[i + k] as usize) << (8 * k);
                    }
                    i += nbytes;
                    len = v + 1;
                }

                if i + len > input.len() {
                    return Err(SnappyError::UnexpectedEof);
                }
                out.extend_from_slice(&input[i..i + len]);
                i += len;
            }

            0b01 => {
                // Copy with 1-byte offset.
                // len in bits [2..4] => 3 bits, [4..11] => len = (v + 4)
                let len = (((tag >> 2) & 0b111) as usize) + 4;

                if i >= input.len() {
                    return Err(SnappyError::UnexpectedEof);
                }
                let off_hi = (tag as usize) >> 5;      // 3 bits
                let off_lo = input[i] as usize;        // 8 bits
                i += 1;
                let offset = (off_hi << 8) | off_lo;   // 11-bit

                copy_from_output(&mut out, offset, len)?;
            }

            0b10 => {
                // Copy with 2-byte offset.
                let len = ((tag >> 2) as usize) + 1; // 1..64
                if i + 2 > input.len() {
                    return Err(SnappyError::UnexpectedEof);
                }
                let offset = (input[i] as usize) | ((input[i + 1] as usize) << 8);
                i += 2;
                copy_from_output(&mut out, offset, len)?;
            }

            _ => {
                // 0b11: Copy with 4-byte offset.
                let len = ((tag >> 2) as usize) + 1;
                if i + 4 > input.len() {
                    return Err(SnappyError::UnexpectedEof);
                }
                let offset = (input[i] as usize)
                    | ((input[i + 1] as usize) << 8)
                    | ((input[i + 2] as usize) << 16)
                    | ((input[i + 3] as usize) << 24);
                i += 4;
                copy_from_output(&mut out, offset, len)?;
            }
        }

        if out.len() > out_len {
            return Err(SnappyError::OutputTooLarge);
        }
    }

    if out.len() != out_len {
        // Not strictly required by all implementations, but it’s a powerful integrity check.
        return Err(SnappyError::OutputTooLarge);
    }

    Ok(out)
}

fn copy_from_output(out: &mut Vec<u8>, offset: usize, len: usize) -> Result<(), SnappyError> {
    if offset == 0 || offset > out.len() {
        return Err(SnappyError::BadOffset);
    }

    // Copy can overlap (RLE-style), so we must copy byte-by-byte if overlap.
    let start = out.len() - offset;

    if offset >= len {
        // Non-overlapping: we can slice-copy.
        let src = out[start..start + len].to_vec();
        out.extend_from_slice(&src);
        return Ok(());
    }

    // Overlapping: classic LZ copy loop.
    for j in 0..len {
        let b = out[start + (j % offset)];
        out.push(b);
    }
    Ok(())
}
```

### Trade-off: Go vs Rust (and why Go can win anyway)

- In Rust, you can write this decoder with tight bounds checks and even SIMD in some cases.
- In Go, the runtime often wins on **branch prediction friendliness** and **inlining heuristics** for byte loops, plus the ecosystem already has heavily optimized Snappy.

The uncomfortable truth: for small payloads, the **constant factors** dominate. A “SIMD-optimized Rust” decoder can lose to Go simply because:

- your Rust code triggers more bounds checks in the hot loop,
- your copies allocate (`to_vec`) (see above),
- or you lose to Go’s tuned assembly.

(If you build a production receiver, you should avoid the `to_vec()` shown here by using careful `extend_from_within` on stable Rust, or a ring-buffered copy strategy.)

## Where cost explodes: label sets, not sample values

In the protobuf structure:

- `Label { name, value }` is **two length-delimited strings**
- every new label adds:
  - tag bytes
  - varint length bytes
  - UTF-8 bytes

For high-cardinality metrics, the compression ratio often collapses because:

- label values are unique (bad for LZ backreferences)
- timestamps are varints but still relatively large

So receivers end up paying:

- more bytes over the network
- more Snappy decode CPU
- more protobuf parse CPU
- more index/storage amplification in the backend

## Architecture comparison: Remote-Write vs OTLP

OTLP is also protobuf, but commonly transported via gRPC or HTTP with gzip support:

- OTLP spec: https://opentelemetry.io/docs/specs/otlp/

The differences that matter in practice:

- Prometheus Remote-Write is *intentionally stateless* per message (no stream semantics), designed to work well over plain HTTP.
- OTLP/gRPC encourages pipelining concurrent unary Export calls for throughput (the spec explicitly describes the throughput bound formula).

### Parquet vs “custom TSDB” (why the wire format influences storage)

If your ingestion wire format is:

- label-heavy,
- small batches,
- and not naturally columnar,

then dumping it into a Parquet lakehouse is usually a mismatch (you pay to “re-columnarize” at ingest). A custom TSDB (Prometheus TSDB, Mimir blocks, etc.) is designed around the fact that the wire format is **series-centric**, not column-centric.

## Research Question (provocation)

Remote-Write 2.0 improves reliability and schema negotiation, but still sends **Snappy-compressed protobuf**.

If Snappy decode + protobuf parse is increasingly the ingestion bottleneck, is the next step:

- *more compression* (zstd) and pay CPU,
- *less parsing* (flatbuffers/cap’n proto) and break compatibility,
- or a paradoxical move: **send more bytes**, but in a layout that’s cheaper to parse (e.g. fixed-width columns per batch), so total cost drops?

Put differently: **When does “worse compression” outperform “better compression” because it is friendlier to CPU pipelines and allocator behavior?**
