# Learn Pin From Failures

[source code](https://github.com/winter-loo/snippets-rust/tree/main/pin)

## Self-referential struct

```rust
#[derive(Debug)]
struct Foo {
    // In somewhat way, we establish the invariant:
    // `a` always points to itself. i.e. `a` represents the start
    // memory address of current Foo instance
    a: *const Foo,
    b: u8,
}
```
