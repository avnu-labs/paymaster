use crate::{Error, ExecutableTransactionParameters};
use paymaster_common::cache::ExpirableCache;
use std::time::Duration;

#[derive(Clone)]
pub struct TransactionDuplicateFilter {
    duplicate_cache: ExpirableCache<u64, ()>,
}

impl Default for TransactionDuplicateFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl TransactionDuplicateFilter {
    pub fn new() -> Self {
        Self {
            duplicate_cache: ExpirableCache::new(1024),
        }
    }

    pub fn filter(&self, transaction: &ExecutableTransactionParameters) -> Result<(), Error> {
        let identifier = transaction.get_unique_identifier();
        if self.duplicate_cache.get_if_not_expired(&identifier).is_some() {
            return Err(Error::Execution("Tx already sent".into()));
        }
        self.duplicate_cache.insert(identifier, (), Duration::from_secs(30));

        Ok(())
    }
}
