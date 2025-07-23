mod shared;
mod workers;

pub use shared::SyncValue;
use thiserror::Error;
use tokio::task::JoinError;
pub use workers::ConcurrentExecutor;

#[derive(Error, Debug)]
pub enum Error {
    /// Indicates a join error on the concurrency Runtime
    #[error(transparent)]
    Join(#[from] JoinError),

    /// Indicates that no workers have been registered
    #[error("no workers")]
    NoWorkers,
}
