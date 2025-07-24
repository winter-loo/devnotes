# tokio spawn

## code path
```text
tokio::spawn
task::spawn_inner
runtime::scheduler::Handle::spawn
runtime::scheduler::multi_thread::Handle::spawn
```

## overview

```rust
/// Spawns a future onto the thread pool
pub(crate) fn spawn<F>(me: &Arc<Self>, future: F, id: task::Id) -> JoinHandle<F::Output>
{
    Self::bind_new_task(me, future, id)
}


fn bind_new_task<T>(me: &Arc<Self>, future: T, id: task::Id) -> JoinHandle<T::Output>
{
    let (handle, notified) = me.shared.owned.bind(future, me.clone(), id);

    me.task_hooks.spawn(&TaskMeta {
        #[cfg(tokio_unstable)]
        id,
        _phantom: Default::default(),
    });

    me.schedule_option_task_without_yield(notified);

    handle
}
```

## data structure

```rust
//: multi_thread::Handle
struct Handle {
    /// Task spawner
    shared: worker::Shared,
}

//: worker::Shared
struct Shared {
    /// Collection of all active tasks spawned onto this executor.
    owned: OwnedTasks<Arc<Handle>>,
}

struct OwnedTasks<S: 'static> {
    list: List<S>,
    id: NonZeroU64,
    closed: AtomicBool,
}
```

## bind task

```rust

impl OwnedTasks {
    fn bind<T>(
        &self,
        task: T,
        scheduler: S,
        id: super::Id,
    ) -> (JoinHandle<T::Output>, Option<Notified<S>>)
    {
        //: see [[task##new task]]
        let (task, notified, join) = super::new_task(task, scheduler, id);
        let notified = unsafe { self.bind_inner(task, notified) };
        (join, notified)
    }

    /// The part of `bind` that's the same for every type of future.
    unsafe fn bind_inner(&self, task: Task<S>, notified: Notified<S>) -> Option<Notified<S>>
    where
        S: Schedule,
    {
        unsafe {
            task.header().set_owner_id(self.id);
        }

        //: this list is a sharded_list::ShardedList
        let shard = self.list.lock_shard(&task);
        if self.closed.load(Ordering::Acquire) {
            drop(shard);
            task.shutdown();
            return None;
        }
        shard.push(task);
        Some(notified)
    }
}
```

[[task#new task|new task]]

[[sharded_list]]
