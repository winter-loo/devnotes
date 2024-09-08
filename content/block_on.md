# block_on

## part 1

[[runtime_intro.md]]

```rust
impl Runtime {
	pub fn block_on<F: Future>(&self, future: F) -> F::Output {
		let _enter = self.enter();
		
		// ...
	}

	pub fn enter(&self) -> EnterGuard<'_> {
        // runtime::Handle::enter
		self.handle.enter()
	}
}

/// module: runtime::Handle
/// Handle to the runtime.
pub struct Handle {
    /// tokio has several schedulers. Each scheduler has its own Handle.
    /// The most important two handles are:
    ///
    ///   - current_thread::Handle
    ///   - multi_thread::Handle
    ///
    pub(crate) inner: scheduler::Handle,
}

// module: runtime::Handle
impl Handle {
	pub fn enter(&self) -> EnterGuard<'_> {
		EnterGuard {
			_guard: match context::try_set_current(&self.inner) {
				Some(guard) => guard,
				None => panic!("{}", crate::util::error::THREAD_LOCAL_DESTROYED_ERROR),
			},
			_handle_lifetime: PhantomData,
		}
	}
}

// module: context::try_set_current
pub(crate) fn try_set_current(handle: &scheduler::Handle) -> Option<SetCurrentGuard> {
    // thread-local context
    CONTEXT.try_with(|ctx| ctx.set_current(handle)).ok()
}

::std::thread_local! {
    static CONTEXT: Context = const {
        Context {
            // Tracks the current runtime handle to use when spawning,
            // accessing drivers, etc...
            #[cfg(feature = "rt")]
            current: current::HandleCell::new(),
        }
    }
}

struct HandleCell {
    /// Current handle
    handle: RefCell<Option<scheduler::Handle>>,

    /// Tracks the number of nested calls to `try_set_current`.
    depth: Cell<usize>,
}

impl Context {
    pub(super) fn set_current(&self, handle: &scheduler::Handle) -> SetCurrentGuard {
        // scheduler::Handle is an Arc type, so `clone` shares the handle
        let old_handle = self.current.handle.borrow_mut().replace(handle.clone());
        let depth = self.current.depth.get();

        assert!(depth != usize::MAX, "reached max `enter` depth");

        let depth = depth + 1;
        self.current.depth.set(depth);

        // old handle will be restored when `SetCurrentGuard` gets dropped
        SetCurrentGuard {
            prev: old_handle,
            depth,
            _p: PhantomData,
        }
    }
}

#[derive(Debug)]
#[must_use]
pub(crate) struct SetCurrentGuard {
    // The previous handle
    prev: Option<scheduler::Handle>,

    // The depth for this guard
    depth: usize,

    // Don't let the type move across threads.
    _p: PhantomData<SyncNotSend>,
}

impl Drop for SetCurrentGuard {
    fn drop(&mut self) {
        CONTEXT.with(|ctx| {
            let depth = ctx.current.depth.get();

            if depth != self.depth {
                return;
            }

            *ctx.current.handle.borrow_mut() = self.prev.take();
            ctx.current.depth.set(depth - 1);
        });
    }
}

#[derive(Debug)]
#[must_use = "Creating and dropping a guard does nothing"]
pub struct EnterGuard<'a> {
    _guard: context::SetCurrentGuard,
    _handle_lifetime: PhantomData<&'a Handle>,
}
```

Summary: assign a `scheduler::Handle` to current thread. When `block_on`
finishes, previous `scheduler::Handle` is restored.

## part 2

```rust
impl Runtime {
	pub fn block_on<F: Future>(&self, future: F) -> F::Output {
		// ...
		
        // we focus on multi-thread scheduler
		match &self.scheduler {
			#[cfg(feature = "rt-multi-thread")]
			Scheduler::MultiThread(exec) => exec.block_on(&self.handle.inner, future),
		}
	}
}

impl MultiThread {
  /// Blocks the current thread waiting for the future to complete.
  ///
  /// The future will execute on the current thread, but all spawned tasks
  /// will be executed on the thread pool.
  pub(crate) fn block_on<F>(&self, handle: &scheduler::Handle, future: F) -> F::Output
  where
    F: Future,
  {
    crate::runtime::context::enter_runtime(handle, true, |blocking| {
      blocking.block_on(future).expect("failed to park thread")
    })

    // REVIEW: the above code could be simplified logically as below:
    // {
    //     use crate::runtime::park::CachedParkThread;
    //     let mut park = CachedParkThread::new();
    //     park.block_on(future)
    // }
  }
}


fn enter_runtime<F, R>(handle: &scheduler::Handle, allow_block_in_place: bool, f: F) -> R
where
    F: FnOnce(&mut BlockingRegionGuard) -> R,
{
    let maybe_guard = CONTEXT.with(|c| {
            // Set the entered flag
            // Here, `runtime` better be renamed `runtime_flag`,
            // `EnterRuntime` better be renamed `EnterRuntimeFlag`
            c.runtime.set(EnterRuntime::Entered {
                allow_block_in_place,
            });

            // Generate a new seed
            let rng_seed = handle.seed_generator().next_seed();

            // Swap the RNG seed
            let mut rng = c.rng.get().unwrap_or_else(FastRand::new);
            let old_seed = rng.replace_seed(rng_seed);
            c.rng.set(Some(rng));

            Some(EnterRuntimeGuard {
                blocking: BlockingRegionGuard::new(),
                handle: c.set_current(handle),
                old_seed,
            })
    });

    // execute function `f` with current settings
    // and restore settings after execution
    if let Some(mut guard) = maybe_guard {
        return f(&mut guard.blocking);
    }

    panic!("...");
}

/// Guard tracking that a caller has entered a runtime context.
#[must_use]
pub(crate) struct EnterRuntimeGuard {
    /// Tracks that the current thread has entered a blocking function call.
    pub(crate) blocking: BlockingRegionGuard,

    #[allow(dead_code)] // Only tracking the guard.
    pub(crate) handle: SetCurrentGuard,

    // Tracks the previous random number generator seed
    old_seed: RngSeed,
}

impl Drop for EnterRuntimeGuard {
    fn drop(&mut self) {
        CONTEXT.with(|c| {
            assert!(c.runtime.get().is_entered());
            c.runtime.set(EnterRuntime::NotEntered);
            // Replace the previous RNG seed
            let mut rng = c.rng.get().unwrap_or_else(FastRand::new);
            rng.replace_seed(self.old_seed.clone());
            c.rng.set(Some(rng));
        });
    }
}

/// Guard tracking that a caller has entered a blocking region.
#[must_use]
pub(crate) struct BlockingRegionGuard {
    _p: PhantomData<NotSendOrSync>,
}

impl BlockingRegionGuard {
    pub(crate) fn block_on<F>(&mut self, f: F) -> Result<F::Output, AccessError>
    where
        F: std::future::Future,
    {
        use crate::runtime::park::CachedParkThread;

        let mut park = CachedParkThread::new();
        park.block_on(f)
    }
}

/// Blocks the current thread using a condition variable.
#[derive(Debug)]
pub(crate) struct CachedParkThread {
    _anchor: PhantomData<NotSendOrSync>,
}

impl CachedParkThread {
    fn block_on<F: Future>(&mut self, f: F) -> Result<F::Output, AccessError> {
        use std::task::Context;
        use std::task::Poll::Ready;

        let waker = self.waker()?;
        let mut cx = Context::from_waker(&waker);

        pin!(f);

        loop {
            // see [[notes 2]]
            if let Ready(v) = crate::runtime::coop::budget(|| f.as_mut().poll(&mut cx)) {
                return Ok(v);
            }

            self.park();
        }
    }

    fn waker(&self) -> Result<Waker, AccessError> {
        // see [[notes 1]]
        self.unpark().map(UnparkThread::into_waker)
    }

    fn unpark(&self) -> Result<UnparkThread, AccessError> {
        self.with_current(ParkThread::unpark)
    }
}
```

**NOTES**

1. see [[design_of_cachedparkthread.md]]
2. see [[what_is_a_cooperative_budget.md]]
3. see [[budget_implementation.md]]
