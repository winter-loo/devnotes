# Send and Sync

## Concept

source: https://doc.rust-lang.org/nomicon/send-and-sync.html

1. Definition:
   - Something is `Send` if it either
     - Does not share mutable state with something else, or
     - Ensures exclusive access to any shared mutable state.
   - a type T is `Sync` if and only if `&T` is `Send`.

2. Automatic Implementation:
   - Most types in Rust are automatically `Send`.
   - Primitive types (like `i32`, `bool`, etc.) are `Send`.
   - Most standard library types are `Send`.
   - Structs and enums are automatically `Send` if all their fields are `Send`.

3. Major exceptions
   - raw pointers are neither `Send` nor `Sync` (because they have no safety guards).
     - raw pointers are, strictly speaking, marked as thread-unsafe as more of a
       lint. If you ensure your struct containing raw pointers is thread-safe,
       you can mark your struct `Send` and `Sync`.
   - `UnsafeCell` isn't `Sync` (and therefore `Cell` and `RefCell` aren't).
   - `Rc` isn't `Send` or `Sync` (because the refcount is shared and unsynchronized).
     - `Rc` and `UnsafeCell` are very fundamentally not thread-safe: they enable
       unsynchronized shared mutable state


## Usage of Send and Sync

### Not Send or Sync

```rust
pub(crate) struct NotSendOrSync(#[allow(dead_code)] *mut ());
```

### Send but not Sync

Here's an example of a type that is `Send` but not `Sync`:

```rust
use std::cell::Cell;

// SendButNotSync automatically Send but not Sync,
// because Cell<T> is Send but not Sync
#[derive(Debug)]
struct SendButNotSync {
    value: Cell<i32>,
}

impl SendButNotSync {
    fn new(initial: i32) -> Self {
        SendButNotSync {
            value: Cell::new(initial),
        }
    }

    fn get(&self) -> i32 {
        self.value.get()
    }

    fn set(&self, new_value: i32) {
        self.value.set(new_value);
    }
}

fn main() {
    let x = SendButNotSync::new(5);
    
    std::thread::spawn(move || {
        println!("Value in new thread: {}", x.get());
        x.set(10);
    }).join().unwrap();
}
```

### Send and Sync

Now, let's demonstrate a type that is both `Send` and `Sync`:

```rust
// SendAndSync is automatically both Send and Sync
// because AtomicI32 is both Send and Sync
struct SendAndSync {
    value: std::sync::atomic::AtomicI32,
}

impl SendAndSync {
    fn new(initial: i32) -> Self {
        SendAndSync {
            value: std::sync::atomic::AtomicI32::new(initial),
        }
    }

    fn get(&self) -> i32 {
        self.value.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn set(&self, new_value: i32) {
        self.value.store(new_value, std::sync::atomic::Ordering::Relaxed);
    }
}

fn main() {
    let x = std::sync::Arc::new(SendAndSync::new(5));
    
    // This is okay because SendAndSync is Send
    // let x_clone = &x; // x can not live long enough
    let x_clone = x.clone();
    std::thread::spawn(move || {
        println!("Value in new thread: {}", x_clone.get());
        x_clone.set(10);
    }).join().unwrap();

    println!("Final value in main thread: {}", x.get());
}
```
