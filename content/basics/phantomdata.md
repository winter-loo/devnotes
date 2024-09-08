# usage of PhantomData

## demo

```rust
pub(crate) struct CachedParkThread {
    _anchor: PhantomData<Rc<()>>,
}
```

```rust
pub struct EnterGuard<'a> {
    _guard: context::SetCurrentGuard,
    _handle_lifetime: PhantomData<&'a Handle>,
}
```

```rust
pub(crate) struct BlockingRegionGuard {
    _p: PhantomData<NotSendOrSync>,
}
```

## explain

PhantomData is a zero-sized type used in Rust to indicate that a type logically
owns or uses data of another type, without actually containing it. It's often
used for various compile-time checks and to express certain relationships
between types. Let's explain each usage:

1. CachedParkThread:

   ```rust
   pub(crate) struct CachedParkThread {
       _anchor: PhantomData<Rc<()>>,
   }
   ```

   Here, PhantomData<Rc<()>> is used to indicate that CachedParkThread is not
   safe to send between threads. The Rc<()> type is not Send, so including it
   as a PhantomData field makes CachedParkThread also not Send. This ensures
   that CachedParkThread is only used on the thread it was created on, which
   is important for its intended use with thread-local storage.

2. EnterGuard:

   ```rust
   pub struct EnterGuard<'a> {
       _guard: context::SetCurrentGuard,
       _handle_lifetime: PhantomData<&'a Handle>,
   }
   ```

   In this case, PhantomData<&'a Handle> is used to tie the lifetime 'a to the
   EnterGuard struct without actually storing a reference. This indicates that
   EnterGuard logically holds a reference to a Handle for its entire lifetime,
   even though it doesn't actually store one. This is likely used to ensure
   that the Handle outlives the EnterGuard.

3. BlockingRegionGuard:

   ```rust
   pub(crate) struct BlockingRegionGuard {
       _p: PhantomData<NotSendOrSync>,
   }
   ```

   Here, PhantomData<NotSendOrSync> is used to make BlockingRegionGuard neither
   Send nor Sync. By including it as a PhantomData field, BlockingRegionGuard
   inherits these properties. This ensures that BlockingRegionGuard can't be
   sent between threads or shared between threads, which is probably important
   for its role in managing blocking regions in an asynchronous context.

In all these cases, PhantomData is used to add compile-time guarantees and
express relationships between types without incurring any runtime cost, as
PhantomData takes up no space in memory.

