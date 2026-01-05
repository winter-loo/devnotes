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

We can initialize the raw pointer with `std::ptr::null`.

```rust
impl Foo {
    fn new(i: u8) -> Foo {
        Foo {
            a: std::ptr::null_mut(),
            b: i,
        }
    }

    fn get_b_from_a(&self) -> u8 {
        let mya = unsafe { &*self.a };
        mya.b
    }

    /// ensure `self.a` points to the beginning address of `Foo` instance
    fn check_invariant(&self) {
        assert_eq!(self.a, self as *const Foo);
    }
}
```

Let's create two instances of `Foo`,

```rust
let mut f1 = Foo::new(10); 
f1.a = &f1 as *const Foo;

let mut f2 = Foo::new(20);
f2.a = &f2 as *const Foo;
```

and print some debug information,

```rust
println!("f1 addr: {:p}", &f1);
println!("f1: {:#?}", f1);
println!("f2 addr: {:p}", &f2);
println!("f2: {:#?}", f2);
println!("f1.a.b: {}, f1.b: {}", f1.get_b_from_a(), f1.b);
println!("f2.a.b: {}, f2.b: {}", f2.get_b_from_a(), f2.b);
```

At first, `check_invariant` is passed,

```rust
f1.check_invariant();
f2.check_invariant();
```

Then, we need process `Foo`s,

```rust
fn handle_foo(f1: &mut Foo, f2: &mut Foo) {
    f1.a = f2.a;
    // std::mem::swap(f1, f2);
}
```

We changed the value of `f1.a` which is a memory address of `f1`.

Thus, the `check_invariant` panics now! That's the problem we will try to solve.
We don't want to violate the contract we defined for our `struct Foo`. Or, we want to
prevent misuse of `struct Foo`. If we allow the misuse of memory, it could crash
our program. That's not the claimed safety of Rust program. So how Rust solves
this problem?

## Pin

Add one more abstraction, specifically, add one more layer on pointer. Here's
the picture from [Pin, Unpin, and why Rust needs them](https://blog.cloudflare.com/pin-and-unpin-in-rust/)

![pin pointer](https://cf-assets.www.cloudflare.com/slt3lc6tev37/19Usw25JStox7edODSK287/65d5cd83004234f75437921d68e825a9/pin_diagram.png)

So, let's add Pin type to our [initial code](https://github.com/winter-loo/snippets-rust/blob/main/pin/src/bin/nopin.rs)

![diff_nopin_pin1](notes-tokio/image/diff_nopin_pin1.png)

You see! It's easy to introduce the Pin type to our code: Only change the signature
of `handle_foo` from `&mut Foo` to `&mut Pin<&mut Foo>`. And convert original pointer
to `Pin` pointer, i.e. shadow the original pointer as below,

```rust
let mut f1 = unsafe { Pin::new_unchecked(&mut f1) };
```

At this phase, the `check_invariant` still can not pass. To prevent misuse of
our `struct Foo`, we need make it `!Unpin`, i.e. we need disable auto trait
`Unpin` for `struct Foo`.

![diff_pin1_main](notes-tokio/image/diff_pin1_main.png)

## Reference

1. https://blog.cloudflare.com/pin-and-unpin-in-rust/
2. https://rust-lang.github.io/async-book/04_pinning/01_chapter.html
3. https://fasterthanli.me/articles/pin-and-suffering
