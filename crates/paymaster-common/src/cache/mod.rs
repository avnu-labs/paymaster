use std::hash::Hash;
use std::ops::Deref;
use std::time::{Duration, Instant};

use moka::sync::Cache;

/// Represents data that becomes stale or expired after a given validity period.
///
/// - After `validity`, the value is considered **stale** (may still be usable).
/// - After `2 * validity`, the value is considered **expired** (unusable).
#[derive(Clone)]
pub struct Expirable<T> {
    /// Time at which the value becomes stale but still usable.
    stale_at: Instant,
    /// Time at which the value becomes expired and must not be used.
    expired_at: Instant,
    /// Base validity duration (stale threshold is `validity`, expired is `2 * validity`).
    validity: Duration,
    value: T,
}

impl<T> Deref for Expirable<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> Expirable<T> {
    pub fn new(value: T, validity: Duration) -> Self {
        Self {
            stale_at: Instant::now() + validity,
            expired_at: Instant::now() + validity * 2,
            validity,
            value,
        }
    }

    /// Returns true if the value is stale (past soft expiration).
    pub fn is_stale(&self) -> bool {
        Instant::now() >= self.stale_at
    }

    /// Returns true if the value is expired (past hard expiration).
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expired_at
    }

    pub fn validity(&self) -> Duration {
        self.validity
    }

    /// Consumes the container and returns the inner value.
    pub fn take(self) -> T {
        self.value
    }

    /// Refreshes the value and resets expiration timers.
    pub fn refresh_with(&mut self, value: T) {
        self.stale_at = Instant::now() + self.validity;
        self.expired_at = Instant::now() + self.validity * 2;
        self.value = value;
    }
}

impl<T: Default> Expirable<T> {
    /// Returns an empty `Expirable` with default value, marked as stale and expired.
    pub fn empty(validity: Duration) -> Self {
        let expired_time = Instant::now() - Duration::from_secs(60);
        Self {
            stale_at: expired_time,
            expired_at: expired_time,
            validity,
            value: T::default(),
        }
    }
}

/// A cache with values that can become stale or expired over time.
///
/// - Stale values may still be returned as fallback.
/// - Expired values are considered unusable.
#[derive(Clone)]
pub struct ExpirableCache<K, V> {
    cache: Cache<K, Expirable<V>>,
}

impl<K, V> ExpirableCache<K, V>
where
    K: 'static + Eq + Hash + Send + Sync,
    V: 'static + Clone + Send + Sync,
{
    pub fn new(capacity: u64) -> Self {
        Self { cache: Cache::new(capacity) }
    }

    /// Returns the value if it exists and is not stale.
    pub fn get_if_not_stale(&self, key: &K) -> Option<V> {
        self.cache.get(key).filter(|x| !x.is_stale()).map(|x| x.value.clone())
    }

    /// Returns the value if it exists and is not expired.
    pub fn get_if_not_expired(&self, key: &K) -> Option<V> {
        self.cache.get(key).filter(|x| !x.is_expired()).map(|x| x.value.clone())
    }

    pub fn insert(&self, key: K, value: V, validity: Duration) {
        self.cache.insert(key, Expirable::new(value, validity));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_is_not_expired() {
        let value = Expirable::new(0, Duration::from_secs(5));
        assert!(!value.is_stale(), "Value should not be stale");
        assert!(!value.is_expired(), "Value should not be expired");
    }

    #[test]
    fn value_is_stale_and_expired_immediately() {
        let value = Expirable::new(0, Duration::ZERO);
        assert!(value.is_stale(), "Value should be stale");
        assert!(value.is_expired(), "Value should be expired");
    }

    #[test]
    fn value_becomes_expired_after_2x_validity() {
        let value = Expirable::new(0, Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(3));
        assert!(value.is_stale(), "Value should be stale after validity");
        assert!(value.is_expired(), "Value should be expired after 2x validity");
    }

    #[test]
    fn cache_get_if_not_stale_returns_value() {
        let cache = ExpirableCache::new(20);
        cache.insert(42, 42, Duration::from_secs(5));
        assert_eq!(cache.get_if_not_stale(&42), Some(42));
    }

    #[test]
    fn cache_get_if_not_stale_returns_none_if_stale() {
        let cache = ExpirableCache::new(20);
        cache.insert(42, 42, Duration::ZERO);
        assert_eq!(cache.get_if_not_stale(&42), None);
    }

    #[test]
    fn cache_get_if_not_expired_returns_value_even_if_stale() {
        let cache = ExpirableCache::new(20);
        let key = 42;
        let validity = Duration::from_millis(5);

        cache.insert(key, key, validity);
        std::thread::sleep(Duration::from_millis(6)); // past stale_at but not expired_at (expired_at = now + 10ms)
        assert_eq!(cache.get_if_not_expired(&key), Some(42));
    }

    #[test]
    fn cache_get_if_not_expired_returns_none_if_expired() {
        let cache = ExpirableCache::new(20);
        let key = 42;
        cache.insert(key, key, Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(3)); // after 2 * validity
        assert_eq!(cache.get_if_not_expired(&key), None);
    }
}
