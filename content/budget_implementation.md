# Origin

```rust
pub(crate) fn block_on<F: Future>(&mut self, f: F) -> Result<F::Output, AccessError> {
    use std::task::Context;
    use std::task::Poll::Ready;

    let waker = self.waker()?;
    let mut cx = Context::from_waker(&waker);

    pin!(f);

    loop {
        // [[note 1]]
        if let Ready(v) = crate::runtime::coop::budget(|| f.as_mut().poll(&mut cx)) {
            return Ok(v);
        }

        self.park();
    }
}
```

## budget

```rust
/// Runs the given closure with a cooperative task budget. When the function
/// returns, the budget is reset to the value prior to calling the function.
#[inline(always)]
pub(crate) fn budget<R>(f: impl FnOnce() -> R) -> R {
    with_budget(Budget::initial(), f)
}
```

`with_budget` sets initial budget to 128.

```rust
#[inline(always)]
fn with_budget<R>(budget: Budget, f: impl FnOnce() -> R) -> R {
    struct ResetGuard {
        prev: Budget,
    }

    impl Drop for ResetGuard {
        fn drop(&mut self) {
            let _ = context::budget(|cell| {
                cell.set(self.prev);
            });
        }
    }

    #[allow(unused_variables)]
    let maybe_guard = context::budget(|cell| {
        // set thread-local budget to the new budget
        let prev = cell.get();
        cell.set(budget);

        // reset thread-local budget to previous budget after `f` execution
        ResetGuard { prev }
    });

    f()
}
```
