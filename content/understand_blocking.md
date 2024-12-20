# blocking

https://rust-exercises.com/100-exercises/08_futures/05_blocking

```shell
git clone https://github.com/winter-loo/100-exercises-to-learn-rust
git checkout 7fd8c8d3efd0a4719743a69daa046bd8be91b819
cd exercises/08_futures/05_blocking
```

```shell
cd exercises/08_futures/06_async_aware_primitives
```

The [program][1] is blocked and its output is:

```text
spawning task
wait for response
```

Explain:
By default, `#[tokio::test]` uses single-threaded runtime. Also, `recv` of
`std::mpsc::channel` is a blocking call, thus the `pong` task never get polled.

The [program][2] is blocked and its output is:

```text
spawning task
wait for response
pong started
Pong received: pong
pong still running
pong still running
pong still running
pong still running
...
```

Explain:
This time, the multi-threaded tokio runtime is used, but we still use blocking IO
functions, so the program can not end its execution. That's because the current
execution control is in our code instead of tokio runtime. Tokio runtime can not
get a chance to cancel the pong task.

Here's another simplifed demo program:

```rust
use tokio;

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    tokio::spawn(async {
        loop {
            // blocking calls
            std::thread::sleep(std::time::Duration::from_millis(1000));
            println!("wake up");
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(1000));
    println!("exiting...");
}
```

The output is,

```text
Hello, world!kkkkkk
exiting...
wake up
wake up
wake up
wake up
...
```

The below is an async program that will end its execution eventually.

```rust
use tokio;

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    tokio::spawn(async {
        loop {
            // await is a cancellation point
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            println!("wake up");
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(1000));
    println!("exiting...");
}
```

[1]: https://github.com/winter-loo/100-exercises-to-learn-rust/blob/6e446486a688abb0cb293ee32ef2b7d5975dd94e/exercises/08_futures/06_async_aware_primitives/src/lib.rs
[2]: https://github.com/winter-loo/100-exercises-to-learn-rust/blob/c7d3eca98324cbba60f9f22eed3b85389ac49f98/exercises/08_futures/06_async_aware_primitives/src/lib.rs
