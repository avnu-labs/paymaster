use std::sync::Arc;
use std::time::Duration;

use futures_core::future::BoxFuture;
use tokio::sync::RwLock;

use crate::cache::Expirable;

/// A SyncValue is a value that can be concurrently read and refreshed if it has expired.
#[derive(Clone)]
pub struct SyncValue<T>(Arc<RwLock<Expirable<T>>>);

impl<T: 'static + Default + Clone + Send> SyncValue<T> {
    /// Initializes the value with an empty [`Expirable`], which is immediately marked as stale and expired.
    pub fn new(validity: Duration) -> Self {
        Self(Arc::new(RwLock::new(Expirable::empty(validity))))
    }

    /// Reads the stored value if it's still fresh (i.e., not stale).
    /// If the value is stale, attempts to refresh it using the provided asynchronous closure.
    ///
    /// - If the value is still usable and the refresh succeeds, returns the fresh value.
    /// - If the refresh fails:
    ///   - and the value is only stale (not expired), returns the stale value.
    ///   - and the value is expired, returns the error.
    pub async fn read_or_refresh<E>(&self, fetch_value: impl FnOnce() -> BoxFuture<'static, Result<T, E>>) -> Result<T, E> {
        let read_lock = self.0.read().await;
        if !read_lock.is_stale() {
            return Ok(read_lock.clone().take());
        }

        drop(read_lock); // Upgrade to write lock

        let mut write_lock = self.0.write().await;
        if !write_lock.is_stale() {
            return Ok(write_lock.clone().take());
        }

        match fetch_value().await {
            Ok(value) => {
                *write_lock = Expirable::new(value.clone(), write_lock.validity());
                Ok(value)
            },
            Err(err) => {
                if write_lock.is_expired() {
                    Err(err)
                } else {
                    Ok(write_lock.clone().take())
                }
            },
        }
    }

    /// Reads the stored value if it's still fresh (i.e., not stale).
    /// If the value is stale, attempts to refresh it using the provided asynchronous closure.
    ///
    /// This variant allows setting a custom TTL (time-to-live) for the refreshed value,
    /// enabling dynamic validity durations based on the fetched result.
    ///
    /// - If the current value is fresh, returns it directly.
    /// - If the value is stale:
    ///   - Attempts to refresh the value using the provided closure, which must return both the value and its TTL.
    ///   - If the refresh succeeds, updates the internal value with the new TTL and returns it.
    ///   - If the refresh fails:
    ///     - and the current value is not expired, returns the stale value.
    ///     - and the value is expired, returns the error.
    pub async fn read_or_refresh_with_ttl<E>(&self, fetch_value: impl FnOnce() -> BoxFuture<'static, Result<(T, u64), E>>) -> Result<T, E> {
        let read_lock = self.0.read().await;
        if !read_lock.is_stale() {
            return Ok(read_lock.clone().take());
        }

        drop(read_lock); // Upgrade to write lock

        let mut write_lock = self.0.write().await;
        if !write_lock.is_stale() {
            return Ok(write_lock.clone().take());
        }

        match fetch_value().await {
            Ok((value, ttl)) => {
                *write_lock = Expirable::new(value.clone(), Duration::from_secs(ttl));
                Ok(value)
            },
            Err(err) => {
                if write_lock.is_expired() {
                    Err(err)
                } else {
                    Ok(write_lock.clone().take())
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::concurrency::SyncValue;

    #[tokio::test]
    async fn read_expired_value() {
        let sync = SyncValue::<i32>::new(Duration::from_secs(10));

        let result = sync.read_or_refresh(|| Box::pin(async { Ok::<i32, ()>(42) })).await;
        assert_eq!(result, Ok(42))
    }

    #[tokio::test]
    async fn read_not_expired_value() {
        let sync = SyncValue::<i32>::new(Duration::from_secs(10));

        let result = sync.read_or_refresh(|| Box::pin(async { Ok::<i32, ()>(42) })).await;
        assert_eq!(result, Ok(42));

        let result = sync.read_or_refresh(|| Box::pin(async { Ok::<i32, ()>(84) })).await;
        assert_eq!(result, Ok(42))
    }
}
