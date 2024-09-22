# local queue

`queue::Local` is a fixed-size(256) array with head, tail, steal indices.

```rust
struct Local<T: 'static> {
    inner: Arc<Inner<T>>,
}

struct Steal<T: 'static>(Arc<Inner<T>>);

struct Inner<T: 'static> {
    /// Concurrently updated by many threads.
    ///
    /// Contains two `UnsignedShort` values. The `LSB` byte is the "real" head of
    /// the queue. The `UnsignedShort` in the `MSB` is set by a stealer in process
    /// of stealing values. It represents the first value being stolen in the
    /// batch. The `UnsignedShort` indices are intentionally wider than strictly
    /// required for buffer indexing in order to provide ABA mitigation and make
    /// it possible to distinguish between full and empty buffers.
    ///
    /// When both `UnsignedShort` values are the same, there is no active
    /// stealer.
    ///
    /// Tracking an in-progress stealer prevents a wrapping scenario.
    head: AtomicU64,

    /// Only updated by producer thread but read by many threads.
    tail: AtomicU32,

    /// Elements
    buffer: Box<[UnsafeCell<MaybeUninit<task::Notified<T>>>; LOCAL_QUEUE_CAPACITY]>,
}
```

[ABA problem](https://spcl.inf.ethz.ch/Teaching/2019-pp/lectures/PP-l21-ConcurrencyTheory.pdf)

![[image/aba2.png]]
![[image/aba1.png]]
