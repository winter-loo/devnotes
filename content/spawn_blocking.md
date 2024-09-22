# spawn_blocking

* This function is intended for non-async operations that eventually finish on
  their own.
* Tokio will spawn more blocking threads when they are requested through this
  function until the upper limit configured on the `Builder` is reached.

Here's code path in top down stack:

```txt
task::blocking::spawn_blocking
runtime::blocking::pool::spawn_blocking
runtime::handle::Handle::spawn_blocking
runtime::blocking::pool::Spawner::spawn_blocking
runtime::blocking::pool::Spawner::spawn_blocking_inner
runtime::blocking::pool::Spawner::spawn_task
```

## pool::spawn_blocking

```rust
fn spawn_blocking<F, R>(func: F) -> JoinHandle<R>
{
    let rt = Handle::current();
    rt.spawn_blocking(func)
}
```

Use current runtime handle for `spawn_blocking` task.

## Handle::spawn_blocking

```rust
impl Handle {
    fn spawn_blocking<F, R>(&self, func: F) -> JoinHandle<R>
    {
        self.inner.blocking_spawner().spawn_blocking(self, func)
    }
}
```

Different schedulers could have different blocking `Spawner`, but tokio
use one `Spawner`.

## Spawner::spawn_blocking

```rust
impl Spawner {
    fn spawn_blocking<F, R>(&self, rt: &runtime::handle::Handle, func: F) -> JoinHandle<R>
    {
        let (join_handle, spawn_result) =
                self.spawn_blocking_inner(Box::new(func), Mandatory::NonMandatory, None, rt)

        match spawn_result {
            Ok(()) => join_handle,
            Err(SpawnError::ShuttingDown) => join_handle,
            Err(SpawnError::NoThreads(e)) => {
                panic!("OS can't spawn worker thread: {}", e)
            }
        }
    }
}

fn spawn_blocking_inner<F, R>(
    &self,
    func: F,
    is_mandatory: Mandatory,
    name: Option<&str>,
    rt: &Handle,
) -> (JoinHandle<R>, Result<(), SpawnError>)
{
    //: `BlockingTask` implements `Future`
    let fut = BlockingTask::new(func);
    let id = task::Id::next();

    //: `task`: task::UnownedTask<BlockingSchedule>
    let (task, handle) = task::unowned(fut, BlockingSchedule::new(rt), id);

    let spawned = self.spawn_task(blocking::pool::Task::new(task, is_mandatory), rt);
    (handle, spawned)
}
```

`BlockingTask` implements `Future`. This future will get polled when [[spawn_task##run task]].

```rust
impl<T, R> Future for BlockingTask<T>
{
    type Output = R;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<R> {
        let me = &mut *self;
        let func = me
            .func
            .take()
            .expect("[internal exception] blocking task ran twice.");

        //: comments removed from the original source code
        crate::runtime::coop::stop();

        Poll::Ready(func())
    }
}
```

[[spawn_task]]

## define an UnownedTask

```rust
/// Converts a function to a future that completes on poll.
struct BlockingTask<T> {
    func: Option<T>,
}

fn unowned<T, S>(task: T, scheduler: S, id: Id) -> (UnownedTask<S>, JoinHandle<T::Output>)
{
    let (task, notified, join) = new_task(task, scheduler, id);

    let unowned = UnownedTask {
        raw: task.raw,
        _p: PhantomData,
    };
    //: takes ownership and do not invoke `task::Task` destructor
    //: Task has `impl Drop for task::Task<S>`
    std::mem::forget(task);
    //: takes ownership and do not invoke `task::Task` destructor
    std::mem::forget(notified);

    (unowned, join)
}
```

[[task#new task|new task]]

