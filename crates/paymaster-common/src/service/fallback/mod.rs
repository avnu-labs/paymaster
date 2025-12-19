use std::sync::Arc;
use std::time::Duration;

use failsafe::backoff::Exponential;
use failsafe::failure_policy::{consecutive_failures, ConsecutiveFailures};
use failsafe::futures::CircuitBreaker;
pub use failsafe::FailurePredicate;
use failsafe::{backoff, Config, StateMachine};
use futures_core::TryFuture;

pub type Error<E> = failsafe::Error<E>;
type FailurePolicy = ConsecutiveFailures<Exponential>;

struct Fallback<T> {
    value: Arc<T>,
    state_machine: StateMachine<FailurePolicy, ()>,
}

impl<T: Clone> Clone for Fallback<T> {
    fn clone(&self) -> Self {
        Self {
            value: Arc::from(self.value.as_ref().clone()),
            state_machine: self.state_machine.clone(),
        }
    }
}

impl<E, T: FailurePredicate<E>> FailurePredicate<E> for &Fallback<T> {
    fn is_err(&self, err: &E) -> bool {
        self.value.is_err(err)
    }
}

impl<T> Fallback<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: value.into(),

            state_machine: Config::new()
                .failure_policy(consecutive_failures(3, backoff::exponential(Duration::from_secs(10), Duration::from_secs(60))))
                .build(),
        }
    }

    async fn call<F>(&self, f: impl FnOnce(Arc<T>) -> F) -> Result<F::Ok, Error<F::Error>>
    where
        F: TryFuture,
        T: FailurePredicate<F::Error>,
    {
        self.state_machine.call_with(self, f(self.value.clone())).await
    }

    fn is_call_permitted(&self) -> bool {
        self.state_machine.is_call_permitted()
    }
}

#[derive(Clone)]
pub struct WithFallback<T> {
    values: Vec<Fallback<T>>,
}

impl<T> Default for WithFallback<T> {
    fn default() -> Self {
        Self { values: vec![] }
    }
}

impl<T> WithFallback<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, alternative: T) -> Self {
        self.values.push(Fallback::new(alternative));
        self
    }

    /// Attempts to execute a function with the first available fallback value.
    ///
    /// This method iterates through the configured fallback values and attempts to
    /// execute the provided function with the first value that is permitted to be called
    /// (i.e., not in a circuit breaker open state). If a value is found and the call
    /// is permitted, it executes the function and returns immediately, regardless of
    /// whether the call succeeds or fails.
    ///
    /// # Arguments
    ///
    /// * `f` - A function that takes an `Arc<T>` and returns a future that can fail
    ///
    /// # Returns
    ///
    /// * `Ok(F::Ok)` - If a permitted value was found and the function executed successfully
    /// * `Err(Error::Rejected)` - If no fallback values are available or all are circuit-broken
    ///
    /// # Type Parameters
    ///
    /// * `F` - A future type that implements `TryFuture`
    /// * `T` - Must implement `FailurePredicate<F::Error>` to determine what constitutes a failure
    pub async fn call<F>(&self, f: impl FnOnce(Arc<T>) -> F) -> Result<F::Ok, Error<F::Error>>
    where
        F: TryFuture,
        T: FailurePredicate<F::Error> + Clone,
    {
        for value in self.values.iter() {
            if value.is_call_permitted() {
                return value.call(f).await;
            }
        }

        Err(Error::Rejected)
    }

    /// Attempts to execute a function with all available fallback values until one succeeds.
    ///
    /// This method iterates through all configured fallback values and attempts to execute
    /// the provided function with each value that is permitted to be called (i.e., not in
    /// a circuit breaker open state). Unlike `call()`, this method will try multiple fallback
    /// values if earlier ones fail, continuing until it finds one that succeeds or all
    /// permitted values have been exhausted.
    ///
    /// # Arguments
    ///
    /// * `f` - A function that takes an `Arc<T>` and returns a future that can fail.
    ///   Note: This must be `Fn` (not `FnOnce`) since it may be called multiple times
    ///
    /// # Returns
    ///
    /// * `Ok(F::Ok)` - If any fallback value succeeded in executing the function
    /// * `Err(Error::Rejected)` - If no fallback values are available, all are circuit-broken,
    ///   or all permitted values failed to execute successfully
    ///
    /// # Type Parameters
    ///
    /// * `F` - A future type that implements `TryFuture`
    /// * `T` - Must implement `FailurePredicate<F::Error>` to determine what constitutes a failure
    ///
    /// # Behavior
    ///
    /// The function will:
    /// 1. Skip any fallback values that are not permitted (circuit breaker is open)
    /// 2. Try each permitted value in sequence
    /// 3. Return immediately upon the first successful execution
    /// 4. Continue to the next value if the current one fails
    /// 5. Return `Error::Rejected` only if all values fail or none are permitted
    pub async fn call_all<F>(&self, f: impl Fn(Arc<T>) -> F) -> Result<F::Ok, Error<F::Error>>
    where
        F: TryFuture,
        T: FailurePredicate<F::Error> + Clone,
    {
        for value in self.values.iter() {
            if !value.is_call_permitted() {
                continue;
            }

            if let Ok(value) = value.call(&f).await {
                return Ok(value);
            }
        }

        Err(Error::Rejected)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use failsafe::FailurePredicate;

    use crate::service::fallback::WithFallback;

    #[derive(Debug)]
    struct Error;

    #[derive(Clone)]
    struct DummyClient(Arc<dyn Fn(usize) -> Result<usize, Error>>);

    impl DummyClient {
        fn execute(&self, i: usize) -> Result<usize, Error> {
            self.0(i)
        }
    }

    impl FailurePredicate<Error> for DummyClient {
        fn is_err(&self, _err: &Error) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn empty_executor_rejects() {
        let executor: WithFallback<DummyClient> = WithFallback::new();

        let result = executor.call(|_| async { Ok(42) }).await;
        assert!(result.is_err())
    }

    #[tokio::test]
    async fn one_value_executor_accepts() {
        let executor = WithFallback::new().with(DummyClient(Arc::new(|_| Ok(0))));

        let result = executor.call(|x| async move { x.execute(0) }).await;
        assert!(result.is_ok())
    }

    #[tokio::test]
    async fn executor_with_failure_reject_once() {
        let executor = WithFallback::new()
            .with(DummyClient(Arc::new(|_| Err(Error))))
            .with(DummyClient(Arc::new(|_| Ok(0))));

        let result = executor.call(|x| async move { x.execute(0) }).await;
        assert!(result.is_err())
    }

    #[tokio::test]
    async fn executor_with_failure_fallback() {
        let executor = WithFallback::new()
            .with(DummyClient(Arc::new(|_| Err(Error))))
            .with(DummyClient(Arc::new(|_| Ok(0))));

        loop {
            let result = executor.call(|x| async move { x.execute(0) }).await;
            if result.is_ok() {
                break;
            }
        }
    }

    #[tokio::test]
    async fn executor_with_failure_fallback_and_recover() {
        let executor = WithFallback::new()
            .with(DummyClient(Arc::new(|i| if i == 0 { Err(Error) } else { Ok(42) })))
            .with(DummyClient(Arc::new(|_| Ok(40))));

        let mut i = 0;
        loop {
            let result = executor.call(|x| async move { x.execute(i) }).await;
            match result {
                Ok(42) => break,
                Ok(_) => i = 1,
                _ => {},
            }
        }
    }
}
