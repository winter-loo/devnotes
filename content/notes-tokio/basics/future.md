# Implement Future

## Contract

* [Future](https://doc.rust-lang.org/std/future/trait.Future.html) is a regular trait which has a method `poll`
* `poll` should return `Poll::Ready(...)` if you think the `Future` complets.
* `poll` should return `Poll::Pending` if you think the `Future` is in progress
* `poll` is initially invoked underneath by the async runtime.
* You need to tell the async runtime to invoke `poll` again if your `Future` is
  in progress. Otherwise, it never complete.

We will use [tokio] as the async runtime.

## Poll::Ready

[The program](https://github.com/winter-loo/snippets-rust/blob/main/future/src/bin/poll_ready.rs) completes immediately.

## Poll::Pending

[The program](https://github.com/winter-loo/snippets-rust/blob/main/future/src/bin/poll_pending.rs) never stop.

How do we tell the async runtime invoke `poll` again?

## Use Waker

### busy polling

[The program](https://github.com/winter-loo/snippets-rust/blob/main/future/src/bin/spin.rs) keeps polling.

```rust
cx.waker().wake_by_ref();
```

[Waker](https://doc.rust-lang.org/std/task/struct.Waker.html) is implemented by
[tokio]

### final complete

Finally, [the program](https://github.com/winter-loo/snippets-rust/blob/main/future/src/bin/final_complete.rs) needs return `Poll::Ready`.

### interval polling

[The program](https://github.com/winter-loo/snippets-rust/blob/main/future/src/bin/interval_polling.rs) keeps interval polling using a thread.

## Use OS mechanism

Thread spawning is not costless. We could use OS timer somehow, but it's a bit
of complex to setup OS layer. Luckily, [tokio Sleep] has already been implemented
for us.

* [this is unsafe code](https://github.com/winter-loo/snippets-rust/blob/main/future/src/bin/tokio_sleep_unsafe.rs)

  We declare a heap-allocated pointer first, then convert that to a Pin pointer.

* [this is safe code](https://github.com/winter-loo/snippets-rust/blob/main/future/src/bin/tokio_sleep_safe.rs)

  We declare a Pin pointer directly using `Box::pin` which is type `Pin<Box<T>>`.
  `Pin<Box<T>>` could be converted to `Pin<&mut T>` using its `Box::as_mut` method.

For how to use `Pin`, see [[pin]]

## Reference

1. [Pin, Unpin, and why Rust needs them] 
2. [Pin and suffering]

[Pin, Unpin, and why Rust needs them]: https://blog.cloudflare.com/pin-and-unpin-in-rust/
[Pin and suffering]: https://fasterthanli.me/articles/pin-and-suffering
[tokio]: https://docs.rs/tokio/latest/tokio/runtime/struct.Runtime.html
[tokio sleep]: https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html
