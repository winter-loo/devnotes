## demo

```rust
#[tokio::main]
async fn main() {
    println!("Hello world");
}
```

Equivalent code not using `#[tokio::main]`

```rust
fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime")
        .block_on(async {
            println!("Hello world");
        })
}
```

## build_threaded_runtime

[source code](https://github.com/tokio-rs/tokio/blob/70569bd0090a3f64aa23c5fccc1d434e04bd90b0/tokio/src/runtime/builder.rs#L1245)

Builder::build() 创建一个 [Runtime](./runtime_intro.md) 实例。

build -> build_threaded_runtime

build_threaded_runtime 函数初始化了三个模块：

1. Driver
2. BlockingPool
3. ThreadedScheduler

### Driver

Driver 里面封装了不同平台的 IO 事件处理机制（epoll, kqueue, IOCP），信号处理机
制，timer 机制。

```text
build_threaded_runtime
runtime::driver::Driver::new
create_io_stack
runtime::io::Driver::new
```

`runtime::io::Driver::new` uses [mio](https://github.com/tokio-rs/mio) to create a new event loop.

### BlockingPool

BlockingPool 实现了线程池机制, 线程池处理一堆在 VecDeque 里的 Task.

### ThreadedScheduler

ThreadedScheduler 包含固定线程数，每个 Worker 线程有个 LocalQueue, 还有一个
SchedulerHandle 用于管理内部线程，如：unpark/wakeup/signal Worker.

在 build_threaded_runtime 的最后，启动所有的 Worker. 每个 Worker 内部运行
Context::run 方法直至 shutdown.

[[worker_runloop]]

## block_on

[[block_on]]
