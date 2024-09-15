# spawn task

## Thread Spawner definition

```rust
pub(crate) struct Spawner {
    inner: Arc<Inner>,
}

struct Inner {
    /// State shared between worker threads.
    shared: Mutex<Shared>,

    /// Pool threads wait on this.
    condvar: Condvar,

    // Maximum number of threads.
    thread_cap: usize,

    // Metrics about the pool.
    metrics: SpawnerMetrics,
}

struct Shared {
    queue: VecDeque<blocking::pool::Task>,
    num_notify: u32,
    shutdown: bool,
    shutdown_tx: Option<shutdown::Sender>,
    /// This holds the `JoinHandles` for all running threads; on shutdown, the thread
    /// calling shutdown handles joining on these.
    worker_threads: HashMap<usize, thread::JoinHandle<()>>,
    /// This is a counter used to iterate `worker_threads` in a consistent order (for loom's
    /// benefit).
    worker_thread_index: usize,
}
```

## spawn_task

If there are idle threads,

```rust
fn spawn_task(&self, task: blocking::pool::Task, rt: &Handle) -> Result<(), SpawnError> {
    let mut shared = self.inner.shared.lock();
    shared.queue.push_back(task);
    self.inner.metrics.inc_queue_depth();
    self.inner.metrics.dec_num_idle_threads();
    shared.num_notify += 1;
    self.inner.condvar.notify_one();
    Ok(())
}
```

If there are no idle threads and we can spawn one,

```rust
fn spawn_task(&self, task: blocking::pool::Task, rt: &Handle) -> Result<(), SpawnError> {
    let mut shared = self.inner.shared.lock();
    shared.queue.push_back(task);
    self.inner.metrics.inc_queue_depth();
    let shutdown_tx = shared.shutdown_tx.clone();

    if let Some(shutdown_tx) = shutdown_tx {
        let id = shared.worker_thread_index;

        match self.spawn_thread(shutdown_tx, rt, id) {
            Ok(handle) => {
                self.inner.metrics.inc_num_threads();
                shared.worker_thread_index += 1;
                shared.worker_threads.insert(id, handle);
            }
            Err(_) => return Err(SpawnError::NoThreads(e));
        }
    }
    Ok(())
}

fn spawn_thread(
    &self,
    shutdown_tx: shutdown::Sender,
    rt: &runtime::handle::Handle,
    id: usize,
) -> io::Result<thread::JoinHandle<()>> {
    let mut builder = thread::Builder::new();

    let rt = rt.clone();

    //: spawn and run
    builder.spawn(move || {
        // Only the reference should be moved into the closure
        let _enter = rt.enter();
        rt.inner.blocking_spawner().inner.run(id);
        drop(shutdown_tx);
    })
}
```

## spawner runloop

```rust
impl Inner {
    fn run(&self, worker_thread_id: usize) {
        let mut shared = self.shared.lock();
        let mut join_on_thread = None;

        'main: loop {
            // BUSY
            while let Some(task) = shared.queue.pop_front() {
                self.metrics.dec_queue_depth();
                drop(shared);
                task.run();

                shared = self.shared.lock();
            }

            // IDLE
            self.metrics.inc_num_idle_threads();

            while !shared.shutdown {
                let timedout = Duration::from_secs(1);
                let lock_result = self.condvar.wait_timeout(shared, timedout).unwrap();

                shared = lock_result.0;
                let timeout_result = lock_result.1;

                if shared.num_notify != 0 {
                    shared.num_notify -= 1;
                    break;
                }

                //: cleanup logic removed when condvar "timed out"

                // Spurious wakeup detected, go back to sleep.
            }

            //: shutdown logic removed
        }

        // Thread exit
        self.metrics.dec_num_threads();

        let _ = self.metrics.dec_num_idle_threads();

        if shared.shutdown && self.metrics.num_threads() == 0 {
            self.condvar.notify_one();
        }

        drop(shared);
    }
}
```
