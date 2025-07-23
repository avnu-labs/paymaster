use futures_core::future::BoxFuture;
use tokio::task::JoinSet;
use tracing::Instrument;

use crate::concurrency::Error;

/// Convenience macro to create task that can be registered in the [`ConcurrentExecutor`].
/// It wraps the given block into a BoxFuture and move the environment into the block
#[macro_export]
macro_rules! task {
    (|$n: ident| $e: block) => {
        move |$n| { Box::pin(async move $e) }
    };
    (|_| $e: block) => {
        move |_| { Box::pin(async move $e) }
    };
}

/// Concurrent task queue that can be used to run multiple tasks in parallel
/// Example
/// ```rust
///  use paymaster_common::concurrency::ConcurrentExecutor;
///  use paymaster_common::task;
///
///  let mut executor = ConcurrentExecutor::new((), 8);
///  executor.register(task!(|_| { 1 }));
///  executor.register(task!(|_| { 1 }));
///
///  let result = executor.execute().await;
/// ```
pub struct ConcurrentExecutor<C, S> {
    context: C,
    n_workers: usize,

    workers: JoinSet<S>,
    queue: Vec<Box<dyn FnOnce(C) -> BoxFuture<'static, S> + Send + Sync>>,
}

impl<C: Clone, S: 'static + Send + Sync> ConcurrentExecutor<C, S> {
    /// Create a new executor with n_workers which means that only n_workers task will be
    /// spawn in parallel. Once a task is done, a new one will be spawn until no remaining
    /// tasks are left. The given context will be implicitly passed to each task using clone.
    pub fn new(context: C, n_workers: usize) -> Self {
        Self {
            context,
            n_workers,

            workers: JoinSet::new(),
            queue: Vec::new(),
        }
    }

    /// Register a new task. Each task must have the same type signature. The macro
    /// [`task!`] can be used to improve readability. Note if there is a worker available
    /// the task will immediately start
    pub fn register<F>(&mut self, task: F) -> &mut Self
    where
        F: 'static + FnOnce(C) -> BoxFuture<'static, S>,
        F: Send + Sync,
    {
        if self.workers.len() >= self.n_workers {
            self.queue.push(Box::new(task));
        } else {
            self.workers.spawn(task(self.context.clone()).in_current_span());
        }

        self
    }

    /// Wait for a task to complete and return the result. A new task will be
    /// automatically started if there is one.
    /// Errors
    ///  - [`Error::Join`] indicates that the task could not be joined properly
    ///  - [`Error::NoWorkers`] indicates that n_workers was set to 0
    pub async fn next(&mut self) -> Option<Result<S, Error>> {
        if self.n_workers == 0 {
            return Some(Err(Error::NoWorkers));
        }

        let value = match self.workers.join_next().await {
            Some(Ok(value)) => Some(Ok(value)),
            None => None,
            Some(Err(e)) => return Some(Err(Error::Join(e))),
        };

        if let Some(task) = self.queue.pop() {
            self.workers.spawn(task(self.context.clone()).in_current_span());
        }

        value
    }

    /// Execute all the registered tasks and return all the results. Note that
    /// the result are returned in no particular order. This method only returns an error
    /// if one of the task cannot be joined properly. Internally, this method calls
    /// next repetitively until no more tasks are waiting and all have been completely executed
    pub async fn execute(&mut self) -> Result<Vec<S>, Error> {
        let mut results = Vec::with_capacity(self.n_workers);
        while let Some(value) = self.next().await {
            results.push(value?)
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use crate::concurrency::ConcurrentExecutor;

    #[tokio::test]
    pub async fn empty_executor() {
        let mut executor = ConcurrentExecutor::new((), 5);
        let values: Vec<u8> = executor.execute().await.unwrap();

        assert!(values.is_empty());
    }

    #[tokio::test]
    pub async fn no_workers_executor() {
        let mut executor = ConcurrentExecutor::new((), 0);
        executor.register(task!(|_| { 5 }));

        let result = executor.execute().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    pub async fn execute_twice_does_not_fail() {
        let mut executor = ConcurrentExecutor::new((), 5);
        executor.register(task!(|_| { 5 }));
        executor.register(task!(|_| { 6 }));

        let mut values = executor.execute().await.unwrap();
        values.sort();

        assert_eq!(values, vec![5, 6]);

        let values = executor.execute().await.unwrap();
        assert!(values.is_empty());
    }

    #[tokio::test]
    pub async fn less_workers_than_task() {
        let mut executor = ConcurrentExecutor::new((), 5);
        executor.register(task!(|_| { 5 }));
        executor.register(task!(|_| { 6 }));
        executor.register(task!(|_| { 7 }));
        executor.register(task!(|_| { 8 }));
        executor.register(task!(|_| { 9 }));
        executor.register(task!(|_| { 10 }));
        executor.register(task!(|_| { 11 }));
        executor.register(task!(|_| { 12 }));

        let mut values = executor.execute().await.unwrap();
        values.sort();

        assert_eq!(values, vec![5, 6, 7, 8, 9, 10, 11, 12]);
    }

    #[tokio::test]
    pub async fn execute_by_step() {
        let mut executor = ConcurrentExecutor::new((), 5);
        executor.register(task!(|_| { 5 }));
        executor.register(task!(|_| { 6 }));
        executor.register(task!(|_| { 7 }));
        executor.register(task!(|_| { 8 }));
        executor.register(task!(|_| { 9 }));
        executor.register(task!(|_| { 10 }));
        executor.register(task!(|_| { 11 }));
        executor.register(task!(|_| { 12 }));

        let mut values = vec![];
        while let Some(result) = executor.next().await {
            values.push(result.unwrap())
        }

        values.sort();
        assert_eq!(values, vec![5, 6, 7, 8, 9, 10, 11, 12]);
    }
}
