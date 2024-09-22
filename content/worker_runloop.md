# worker runloop

[source](https://github.com/tokio-rs/tokio/blob/a302367b8f80ca48979c4e536a9ae79c1b3d640d/tokio/src/runtime/scheduler/multi_thread/worker.rs#L471)

## overview

```rust
/// A scheduler worker
pub(super) struct Worker {
    /// Reference to scheduler's handle
    handle: Arc<Handle>,

    /// Index holding this worker's remote state
    index: usize,

    /// Used to hand-off a worker's core to another thread.
    core: AtomicCell<Core>,
}

/// State shared across all workers
pub(crate) struct Shared {
    /// Global task queue used for:
    ///  1. Submit work to the scheduler while **not** currently on a worker thread.
    ///  2. Submit work to the scheduler when a worker run queue is saturated
    pub(super) inject: inject::Shared<Arc<Handle>>,

    /// Coordinates idle workers
    idle: Idle,

    /// Collection of all active tasks spawned onto this executor.
    pub(crate) owned: OwnedTasks<Arc<Handle>>,

    /// Data synchronized by the scheduler mutex
    pub(super) synced: Mutex<Synced>,

    /// Cores that have observed the shutdown signal
    ///
    /// The core is **not** placed back in the worker to avoid it from being
    /// stolen by a thread that was spawned as part of `block_in_place`.
    #[allow(clippy::vec_box)] // we're moving an already-boxed value
    shutdown_cores: Mutex<Vec<Box<Core>>>,
}

/// Core data
struct Core {
    tick: u32,
    /// How often to check the global queue
    global_queue_interval: u32,

    /// The worker-local run queue.
    run_queue: queue::Local<Arc<Handle>>,

    /// True if the worker is currently searching for more work. Searching
    /// involves attempting to steal from other workers.
    is_searching: bool,

    /// True if the scheduler is being shutdown
    is_shutdown: bool,

    /// Parker
    ///
    /// Stored in an `Option` as the parker is added / removed to make the
    /// borrow checker happy.
    park: Option<Parker>,

    /// How often to check the global queue
    global_queue_interval: u32,

    /// Fast random number generator.
    rand: FastRand,
}

impl Launch {
    fn launch(mut self) {
        for worker in self.0.drain(..) {
            runtime::spawn_blocking(move || run(worker));
        }
    }
}

fn run(worker: Arc<Worker>) {
    let core = match worker.core.take() {
        Some(core) => core,
        None => return,
    };

    worker.handle.shared.worker_metrics[worker.index].set_thread_id(thread::current().id());

    let handle = scheduler::Handle::MultiThread(worker.handle.clone());

    crate::runtime::context::enter_runtime(&handle, true, |_| {
        // Set the worker context.
        let cx = scheduler::Context::MultiThread(Context {
            worker,
            core: RefCell::new(None),
            defer: Defer::new(),
        });

        context::set_scheduler(&cx, || {
            // This should always be an error. It only returns a `Result` to support
            // using `?` to short circuit.
            //: runtime assertion
            assert!(cx.run(core).is_err());

            // Check if there are any deferred tasks to notify. This can happen when
            // the worker core is lost due to `block_in_place()` being called from
            // within the task.
            cx.defer.wake();
        });
    });
}
```

## Context::run

```rust
impl Context {
    fn run(&self, mut core: Box<Core>) -> RunResult {
        while !core.is_shutdown {
            // Increment the tick
            core.tick();

            // First, check work available to the current worker.
            if let Some(task) = core.next_task(&self.worker) {
                core = self.run_task(task, core)?;
                continue;
            }

            // There is no more **local** work to process, try to steal work
            // from other workers.
            if let Some(task) = core.steal_work(&self.worker) {
                core = self.run_task(task, core)?;
            } else {
                // Wait for work
                core = if !self.defer.is_empty() {
                    self.park_timeout(core, Some(Duration::from_millis(0)))
                } else {
                    self.park(core)
                };
            }
        }

        // Signal shutdown
        self.worker.handle.shutdown_core(core);
        Err(())
    }
}
```

### next_task

* get task from global queue after interval
* or, get task from local queue
* or, get some tasks from global queue if local queue is empty

### steal_task

steal tasks from another worker's local queue.

[[local_queue|local queue]]
