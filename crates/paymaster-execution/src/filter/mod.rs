use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use moka::ops::compute::{CompResult, Op};
use moka::sync::Cache;

use crate::{Error, ExecutableTransactionParameters};

#[derive(Clone)]
pub struct TransactionDuplicateFilter {
    submissions: Cache<u64, u64>,
    next_reservation: Arc<AtomicU64>,
}

impl Default for TransactionDuplicateFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl TransactionDuplicateFilter {
    pub fn new() -> Self {
        Self {
            submissions: Cache::builder()
                .max_capacity(1024)
                .time_to_live(Duration::from_secs(60))
                .build(),
            next_reservation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Reject a transaction already submitted in the last 60 seconds: as the first submission may still
    /// be in flight, its outcome is uncertain.
    pub fn filter(&self, transaction: &ExecutableTransactionParameters) -> Result<Reservation, Error> {
        let transaction = transaction.get_unique_identifier();
        let id = self.next_reservation.fetch_add(1, Ordering::Relaxed);

        // Admission and release both go through `and_compute_with`, which serializes them per transaction:
        // only one concurrent submission is admitted, and a release cannot remove a newer reservation
        let CompResult::Inserted(_) = self.submissions.entry(transaction).and_compute_with(|entry| match entry {
            None => Op::Put(id),
            Some(_) => Op::Nop,
        }) else {
            return Err(Error::DuplicateSubmission);
        };

        Ok(Reservation {
            submissions: self.submissions.clone(),
            transaction,
            id,
        })
    }
}

/// A transaction admitted by [`TransactionDuplicateFilter::filter`]. It is intentionally not released on drop: a
/// request cancelled while submitting may have broadcast its transaction.
#[must_use]
pub struct Reservation {
    submissions: Cache<u64, u64>,
    transaction: u64,
    id: u64,
}

impl Reservation {
    /// Allow the transaction to be submitted again. Must only be called when it was not broadcast. Does nothing if
    /// the reservation expired and was replaced by another submission.
    pub fn release(self) {
        self.submissions.entry(self.transaction).and_compute_with(|entry| match entry {
            Some(entry) if *entry.value() == self.id => Op::Remove,
            _ => Op::Nop,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use moka::ops::compute::Op;
    use starknet::core::types::Felt;

    use crate::{DeploymentParameters, Error, ExecutableTransactionParameters, TransactionDuplicateFilter};

    fn a_deployment() -> ExecutableTransactionParameters {
        ExecutableTransactionParameters::Deploy {
            deployment: DeploymentParameters {
                address: Felt::ONE,
                class_hash: Felt::TWO,
                salt: Felt::THREE,
                unique: Felt::ZERO,
                calldata: vec![],
                sigdata: None,
                version: 1,
            },
        }
    }

    #[test]
    fn duplicate_submission_is_rejected() {
        let filter = TransactionDuplicateFilter::new();
        let transaction = a_deployment();

        let _reservation = filter.filter(&transaction).unwrap();

        assert!(matches!(filter.filter(&transaction), Err(Error::DuplicateSubmission)));
    }

    #[test]
    fn released_transaction_can_be_submitted_again() {
        let filter = TransactionDuplicateFilter::new();
        let transaction = a_deployment();

        filter.filter(&transaction).unwrap().release();

        assert!(filter.filter(&transaction).is_ok());
    }

    #[test]
    fn stale_release_keeps_the_new_submission() {
        let filter = TransactionDuplicateFilter::new();
        let transaction = a_deployment();

        let stale = filter.filter(&transaction).unwrap();
        // Simulate the expiration of the first reservation
        filter.submissions.invalidate(&transaction.get_unique_identifier());
        let _reservation = filter.filter(&transaction).unwrap();

        stale.release();

        assert!(filter.filter(&transaction).is_err());
    }

    /// Regression guard: admitting with `or_insert_with` is not serialized with the release `and_compute_with`, which
    /// lets a release that checked its expired reservation remove the newer one.
    #[test]
    fn admission_waits_for_an_in_flight_release() {
        let filter = &TransactionDuplicateFilter::new();
        let transaction = &a_deployment();
        let identifier = transaction.get_unique_identifier();
        let _reservation = filter.filter(transaction).unwrap();

        thread::scope(|scope| {
            // Created in the scope so that a failed assertion drops them and unblocks the paused release
            let (release_paused, wait_release_paused) = mpsc::channel();
            let (resume_release, wait_resume_release) = mpsc::channel::<()>();
            let (admitted, wait_admitted) = mpsc::channel();

            // A release that checked its reservation, then got paused
            scope.spawn(move || {
                filter.submissions.entry(identifier).and_compute_with(|_| {
                    release_paused.send(()).unwrap();
                    let _ = wait_resume_release.recv();
                    Op::Remove
                })
            });
            wait_release_paused.recv().unwrap();

            // Meanwhile, the reservation expires and the transaction is submitted again
            filter.submissions.invalidate(&identifier);
            scope.spawn(move || admitted.send(filter.filter(transaction).is_ok()).unwrap());
            assert!(wait_admitted.recv_timeout(Duration::from_millis(100)).is_err(), "admission must wait for the release");

            resume_release.send(()).unwrap();
            assert!(wait_admitted.recv().unwrap());
        });

        assert!(filter.filter(transaction).is_err());
    }
}
