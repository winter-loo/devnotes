Let's break down the design of `ParkThread` and `UnparkThread`:

1. Overall Structure:
   Both `ParkThread` and `UnparkThread` share a common `Inner` struct through an `Arc`:

```rust
pub(crate) struct ParkThread {
    inner: Arc<Inner>,
}

pub(crate) struct UnparkThread {
    inner: Arc<Inner>,
}

struct Inner {
    state: AtomicUsize,
    mutex: Mutex<()>,
    condvar: Condvar,
}
```

2. State Management:
   The `Inner` struct uses an `AtomicUsize` to manage the parking state:

```rust
const EMPTY: usize = 0;
const PARKED: usize = 1;
const NOTIFIED: usize = 2;
```

3. ParkThread Functionality:
   - `new()`: Creates a new `ParkThread` with an initialized `Inner`.
   - `unpark()`: Creates an `UnparkThread` from the same `Inner`.
   - `park()`: Parks the current thread.
   - `park_timeout()`: Parks the thread with a timeout.

4. UnparkThread Functionality:
   - `unpark()`: Unparks a parked thread.
   - `into_waker()`: Converts the `UnparkThread` into a `Waker` for use with futures.

5. Parking Mechanism:
   The `park()` method in `Inner`:
   - Uses compare-and-swap operations to manage state transitions.
   - Utilizes a mutex and condition variable for actual thread blocking.
   - Handles spurious wakeups by rechecking the state.

6. Unparking Mechanism:
   The `unpark()` method in `Inner`:
   - Uses atomic operations to update the state.
   - Notifies the condition variable to wake up a parked thread.

7. Thread-Safety:
   The use of atomic operations, mutexes, and condition variables ensures thread-safe operation.

8. Waker Integration:
   `UnparkThread` can be converted into a `Waker`, allowing integration with Rust's async/await system.

9. Efficiency:
   - Fast path for already-notified threads.
   - Atomic operations for quick state checks.
   - Mutex only acquired when actually parking.

10. Flexibility:
    Supports both indefinite parking and timeout-based parking.

This design allows for efficient thread parking and unparking, which is crucial for Tokio's runtime performance. The separation of `ParkThread` and `UnparkThread` allows for clear ownership semantics, where the thread that can park itself holds the `ParkThread`, while other threads that need to wake it up can hold `UnparkThread`s. 
