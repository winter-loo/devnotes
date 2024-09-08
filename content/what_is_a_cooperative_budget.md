A cooperative task budget is a mechanism used in Tokio's runtime to implement
cooperative multitasking. Here's a breakdown of the concept:

1. Purpose:
   The cooperative task budget is designed to prevent long-running tasks from
   monopolizing the CPU, ensuring fair execution time distribution among tasks.

2. How it works:
   - Each task is assigned a budget (a certain number of "work units").
   - The budget is decremented as the task performs operations.
   - When the budget is exhausted, the task is expected to yield control back
     to the scheduler.

3. Implementation:
   In Tokio, the budget is represented by the `Budget` struct:

   ```rust
   pub(crate) struct Budget(Option<u8>);
   ```

   The initial budget is set to 128 units:

   ```rust
   const fn initial() -> Budget {
       Budget(Some(128))
   }
   ```

4. Yielding:
   Tasks can check their remaining budget and yield if necessary:

   ```rust
   pub(crate) fn poll_proceed(cx: &mut Context<'_>) -> Poll<RestoreOnPending>
   ```

   This function decrements the budget and returns `Poll::Pending` if the
   budget is exhausted.

5. Resetting:
   The budget is reset when a task is rescheduled or when explicitly requested.

6. Flexibility:
   The system allows for unconstrained budgets in certain scenarios:

   ```rust
   pub(super) const fn unconstrained() -> Budget {
       Budget(None)
   }
   ```

7. Thread-local storage:
   The budget is stored in thread-local storage, allowing for efficient access
   and modification.

By using this cooperative task budget system, Tokio ensures that tasks play
nicely with each other, preventing any single task from dominating CPU time
and ensuring responsive and fair execution across all tasks in the runtime.
