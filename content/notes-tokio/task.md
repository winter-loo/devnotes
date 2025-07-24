# Task

## new task

```rust
fn new_task<T, S>(
    task: T,
    scheduler: S,
    id: Id,
) -> (Task<S>, Notified<S>, JoinHandle<T::Output>)
{
    //: RawTask has Copy semantics
    //: RawTask includes a pointer to `Header`, i.e. pointer to `Cell`
    //: The pointer points to a heap-allocated value(`Box::new`ed).
    let raw = RawTask::new::<T, S>(task, scheduler, id);
    //: `Task` has `impl Drop`
    let task = task::Task {
        raw, //: raw copied
        _p: PhantomData,
    };
    //: `Task` will be dropped as `Notified` gets dropped
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

struct UnownedTask<S: 'static> {
    raw: RawTask,
    _p: PhantomData<S>,
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

## task deallocation

A task is initialized with three references. Only when ref count decrements
to zero, the underlying `Cell` gets deallocated. Tokio uses `std::mem::forget`
to avoid [double free](https://github.com/winter-loo/tokio-notes/blob/main/code/src/bin/double_free.rs).

```rust
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

struct UnownedTask<S: 'static> {
    raw: RawTask,
    _p: PhantomData<S>,
}


impl<S: 'static> Drop for Task<S> {
    fn drop(&mut self) {
        // Decrement the ref count
        if self.header().state.ref_dec() {
            // Deallocate if this is the final ref count
            self.raw.dealloc();
        }
    }
}

impl<S: 'static> Drop for UnownedTask<S> {
    fn drop(&mut self) {
        // Decrement the ref count
        if self.raw.header().state.ref_dec_twice() {
            // Deallocate if this is the final ref count
            self.raw.dealloc();
        }
    }
}

impl RawTask {
    fn dealloc(self) {
        let vtable = self.header().vtable;
        //: deallocate `Cell`
        unsafe {
            (vtable.dealloc)(self.ptr);
        }
    }
}

/// Typed raw task handle.
struct Harness<T: Future, S: 'static> {
    cell: NonNull<Cell<T, S>>,
}

unsafe fn dealloc<T: Future, S: Schedule>(ptr: NonNull<Header>) {
    let harness = Harness::<T, S>::from_raw(ptr);
    harness.dealloc();
}

impl Harness {
    fn dealloc(self) {
        unsafe {
            drop(Box::from_raw(self.cell.as_ptr()));
        }
    }
}
```
