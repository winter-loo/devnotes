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
    //: takes ownership and do not invoke destructor
    //: Task has `impl Drop for Task<S>`
    std::mem::forget(task);
    //: takes ownership and do not invoke destructor
    std::mem::forget(notified);

    (unowned, join)
}


fn new_task<T, S>(
    task: T,
    scheduler: S,
    id: Id,
) -> (Task<S>, Notified<S>, JoinHandle<T::Output>)
{
    //: RawTask has Copy semantics
    let raw = RawTask::new::<T, S>(task, scheduler, id);
    let task = Task {
        raw, //: raw copied
        _p: PhantomData,
    };
    let notified = Notified(Task {
        raw, //: raw copied
        _p: PhantomData,
    });
    let join = JoinHandle::new(raw); //: raw copied

    (task, notified, join)
}

//: `Task` has `PhantomData<S>` as `RawTask::new` need generic type `S`
//: `RawTask::new` need `S` as `Cell` need `S`
//: `Cell` need `S` as `Core` need `S`
struct Task<S: 'static> {
    raw: RawTask,
    _p: PhantomData<S>,
}

//: new type pattern
struct Notified<S: 'static>(Task<S>);

//: `Header` is the first field of `Cell`
//: pointer to `Header` implies pointer to `Cell`
struct RawTask {
    ptr: NonNull<Header>,
}

struct Header {
    /// Task state.
    pub(super) state: State,

    /// Pointer to next task, used with the injection queue.
    pub(super) queue_next: UnsafeCell<Option<NonNull<Header>>>,

    /// Table of function pointers for executing actions on the task.
    pub(super) vtable: &'static Vtable,
}

struct State {
    val: AtomicUsize,
}

impl RawTask {
    fn new<T, S>(task: T, scheduler: S, id: Id) -> RawTask
    {
        let ptr = Box::into_raw(Cell::<_, S>::new(task, scheduler, State::new(), id));
        //: ptr.cast() casts `Cell` to `Header`
        let ptr = unsafe { NonNull::new_unchecked(ptr.cast()) };

        RawTask { ptr }
    }
}

#[repr(C)]
struct Cell<T: Future, S> {
    /// Hot task state data
    //: It is critical for `Header` to be the first field as the task structure will
    //: be referenced by both *mut Cell and *mut Header.
    header: Header,

    /// Either the future or output, depending on the execution stage.
    core: Core<T, S>,

    /// Cold data
    trailer: Trailer,
}

impl<T: Future, S: Schedule> Cell<T, S> {
    fn new(future: T, scheduler: S, state: State, task_id: Id) -> Box<Cell<T, S>> {
        fn new_header(
            state: State,
            vtable: &'static Vtable,
        ) -> Header {
            Header {
                state,
                queue_next: UnsafeCell::new(None),
                vtable,
                owner_id: UnsafeCell::new(None),
            }
        }

        let vtable = raw::vtable::<T, S>();
        let result = Box::new(Cell {
            trailer: Trailer::new(scheduler.hooks()),
            header: new_header(
                state,
                vtable,
            ),
            core: Core {
                scheduler,
                stage: CoreStage {
                    stage: UnsafeCell::new(Stage::Running(future)),
                },
                task_id,
            },
        });

        result
    }
}

fn vtable<T: Future, S: Schedule>() -> &'static Vtable {
    &Vtable {
        poll: poll::<T, S>,
        schedule: schedule::<S>,
        dealloc: dealloc::<T, S>,
        try_read_output: try_read_output::<T, S>,
        drop_join_handle_slow: drop_join_handle_slow::<T, S>,
        drop_abort_handle: drop_abort_handle::<T, S>,
        shutdown: shutdown::<T, S>,
        trailer_offset: OffsetHelper::<T, S>::TRAILER_OFFSET,
        scheduler_offset: OffsetHelper::<T, S>::SCHEDULER_OFFSET,
        id_offset: OffsetHelper::<T, S>::ID_OFFSET,
    }
}

#[repr(C)]
struct Core<T: Future, S> {
    /// Scheduler used to drive this future.
    scheduler: S,

    /// The task's ID, used for populating `JoinError`s.
    task_id: Id,

    /// Either the future or the output.
    stage: CoreStage<T>,
}

struct CoreStage<T: Future> {
    stage: UnsafeCell<Stage<T>>,
}

#[repr(C)]
enum Stage<T: Future> {
    Running(T),
    Finished(super::Result<T::Output>),
    Consumed,
}
```
