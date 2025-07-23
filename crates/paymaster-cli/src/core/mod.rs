use thiserror::Error;

pub mod context;
pub mod starknet;

#[derive(Error, Debug)]
pub enum Error {
    #[error("CLI execution error: {0}")]
    Execution(String),
    #[error("CLI validation error: {0}")]
    Validation(String),
}
