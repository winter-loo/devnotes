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

## run task

```rust
//: blocking::pool::Task
struct Task {
    task: task::UnownedTask<BlockingSchedule>,
    mandatory: Mandatory,
}

impl Task {
    fn run(self) {
        self.task.run();
    }
}

impl UnownedTask {
    fn run(self) {
        let raw = self.raw;
        mem::forget(self);

        // Transfer one ref-count to a Task object.
        let task = Task::<S> {
            raw,
            _p: PhantomData,
        };

        // Use the other ref-count to poll the task.
        raw.poll();
        // Decrement our extra ref-count
        drop(task);
    }
}

impl RawTask {
    fn poll(self) {
        let vtable = self.header().vtable;
        unsafe { (vtable.poll)(self.ptr) }
    }
}

unsafe fn poll<T: Future, S: Schedule>(ptr: NonNull<Header>) {
    let harness = Harness::<T, S>::from_raw(ptr);
    harness.poll();
}

impl Harness {
    fn poll(self) {
        match self.poll_inner() {
            PollFuture::Notified => {
                self.core()
                    .scheduler
                    .yield_now(Notified(self.get_new_task()));

                self.drop_reference();
            }
            PollFuture::Complete => {
                self.complete();
            }
            PollFuture::Dealloc => {
                self.dealloc();
            }
            PollFuture::Done => (),
        }
    }

    //: poll around state
    fn poll_inner(&self) -> PollFuture {
        use super::state::{TransitionToIdle, TransitionToRunning};

        match self.state().transition_to_running() {
            TransitionToRunning::Success => {
                let header_ptr = self.header_ptr();
                let waker_ref = waker_ref::<S>(&header_ptr);
                let cx = Context::from_waker(&waker_ref);
                let res = poll_future(self.core(), cx);

                if res == Poll::Ready(()) {
                    // The future completed. Move on to complete the task.
                    return PollFuture::Complete;
                }
                match self.state().transition_to_idle() {
                    TransitionToIdle::Ok => PollFuture::Done,
                    TransitionToIdle::OkNotified => PollFuture::Notified,
                    TransitionToIdle::OkDealloc => PollFuture::Dealloc,
                    TransitionToIdle::Cancelled => PollFuture::Complete,
                }
            }
            TransitionToRunning::Cancelled => {
                cancel_task(self.core());
                PollFuture::Complete
            }
            TransitionToRunning::Failed => PollFuture::Done,
            TransitionToRunning::Dealloc => PollFuture::Dealloc,
        }
    }
}


//: poll around panics!
fn poll_future<T: Future, S: Schedule>(core: &Core<T, S>, cx: Context<'_>) -> Poll<()> {
    // Poll the future.
    let output = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        struct Guard<'a, T: Future, S: Schedule> {
            core: &'a Core<T, S>,
        }
        impl<'a, T: Future, S: Schedule> Drop for Guard<'a, T, S> {
            fn drop(&mut self) {
                // If the future panics on poll, we drop it inside the panic
                // guard.
                self.core.drop_future_or_output();
            }
        }
        let guard = Guard { core };
        //: real poll
        let res = guard.core.poll(cx);
        mem::forget(guard);
        res
    }));

    // Prepare output for being placed in the core stage.
    let output = match output {
        Ok(Poll::Pending) => return Poll::Pending,
        Ok(Poll::Ready(output)) => Ok(output),
        Err(panic) => Err(panic_to_error(&core.scheduler, core.task_id, panic)),
    };

    // Catch and ignore panics if the future panics on drop.
    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        core.store_output(output);
    }));

    if res.is_err() {
        core.scheduler.unhandled_panic();
    }

    Poll::Ready(())
}
```

[[task_state]]
