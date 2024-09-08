# The Tokio runtime

_source: runtime.rs_

The runtime provides an I/O driver, task scheduler, timer, and
blocking pool, necessary for running asynchronous tasks.

Instances of `Runtime` can be created using `new`, or `Builder`.
However, most users will use the `#[tokio::main]` annotation on
their entry point instead.

## Shutdown

Shutting down the runtime is done by dropping the value, or calling
`shutdown_background` or `shutdown_timeout`.

Tasks spawned through `Runtime::spawn` keep running until they yield.
Then they are dropped. They are not *guaranteed* to run to completion, but
*might* do so if they do not yield until completion.

Blocking functions spawned through `Runtime::spawn_blocking` keep running
until they return.

The thread initiating the shutdown blocks until all spawned work has been
stopped. This can take an indefinite amount of time. The `Drop`
implementation waits forever for this.

The `shutdown_background` and `shutdown_timeout` methods can be used if
waiting forever is undesired. When the timeout is reached, spawned work that
did not stop in time and threads running it are leaked. The work continues
to run until one of the stopping conditions is fulfilled, but the thread
initiating the shutdown is unblocked.

Once the runtime has been dropped, any outstanding I/O resources bound to
it will no longer function. Calling any method on them will result in an
error.

## Sharing

There are several ways to establish shared access to a Tokio runtime:

 * Using an `Arc<Runtime>`.
 * Using a `Handle`.
 * Entering the runtime context.

Using an `Arc<Runtime>` or `Handle` allows you to do various
things with the runtime such as spawning new tasks or entering the runtime
context. Both types can be cloned to create a new handle that allows access
to the same runtime. By passing clones into different tasks or threads, you
will be able to access the runtime from those tasks or threads.

The difference between `Arc<Runtime>` and `Handle` is that
an `Arc<Runtime>` will prevent the runtime from shutting down,
whereas a `Handle` does not prevent that. This is because shutdown of the
runtime happens when the destructor of the `Runtime` object runs.

Calls to `shutdown_background` and `shutdown_timeout` require exclusive
ownership of the `Runtime` type. When using an `Arc<Runtime>`,
this can be achieved via `Arc::try_unwrap` when only one strong count
reference is left over.

The runtime context is entered using the `Runtime::enter` or
`Handle::enter` methods, which use a thread-local variable to store the
current runtime. Whenever you are inside the runtime context, methods such
as `tokio::spawn` will use the runtime whose context you are inside.
