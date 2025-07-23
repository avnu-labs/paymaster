pub mod fallback;
pub mod messaging;

use async_trait::async_trait;
use thiserror::Error;

mod runner;

pub use runner::{ServiceManager, TokioServiceManager};

pub mod monitoring;

pub use tracing;

use crate::service::messaging::MessageIdentity;

#[macro_export]
macro_rules! log_if_error {
    ($e: expr) => {
        match $e {
            Ok(v) => Ok(v),
            Err(e) => {
                $crate::service::tracing::error!("{}", e);
                Err(e)
            },
        }
    };
}

/// Convenience macro to log a message using the underlying [`MessageIdentity`]
/// Example
/// ```rust
/// use paymaster_common::service::{Error, Service};
///
/// use paymaster_common::service_info;
///
/// pub struct MyService;
///
/// impl Service for MyService {
///     const NAME: &'static str = "MyService";
///     type Context = ();
///
///     async fn new(context: Self::Context) -> Self { todo!() }
///
///     async fn run(self) -> Result<(), Error> {
///         service_info!("foo"); // print `[MyService] foo`
///         Ok(())
///     }
/// }
///
/// ```
#[macro_export]
macro_rules! service_info {
    ($s: literal $(, $v: expr)*) => {
        $crate::log::info!(target: <Self>::NAME , $s, $($v),*);
    };
}

/// Convenience macro to log a message using the underlying [`MessageIdentity`]. See [`service_info`]
#[macro_export]
macro_rules! service_warn {
    ($s: literal $(, $v: expr)*) => {
        $crate::log::warn!(target: <Self>::NAME , $s, $($v),*);
    };
}

/// Convenience macro to log a message using the underlying [`MessageIdentity`]. See [`service_info`]
#[macro_export]
macro_rules! service_error {
    ($s: literal $(, $v: expr)*) => {
        $crate::log::error!(target: <Self>::NAME , $s, $($v),*);
    };
}

/// Convenience macro to log a message using the underlying [`MessageIdentity`]. See [`service_info`]
#[macro_export]
macro_rules! service_debug {
    ($s: literal $(, $v: expr)*) => {
        $crate::log::debug!(target: <Self>::NAME , $s, $($v),*);
    };
}

/// Check if the given value is an error or not.
///  - If Ok just continue normally
///  - If Err then print the error using the underlying [`MessageIdentity`] and execute the expression given to the macro
///
/// Example
/// ```rust
/// use paymaster_common::service::{Error, Service};
///
/// use paymaster_common::{service_check, service_info};
///
/// pub struct MyService;
///
/// impl Service for MyService {
///     const NAME: &'static str = "MyService";
///     type Context = ();
///
///     async fn new(context: Self::Context) -> Self { todo!() }
///
///     async fn run(self) -> Result<(), Error> {
///         service_check!(Ok(()) => return Err(Error::new("dummy"))); // Do nothing
///         service_check!(Err(Error::new("foo")) => return Err(Error::new("bar"))); // Print `foo` and return Error::new("bar")
///
///         Ok(())
///     }
/// }
///
/// ```
#[macro_export]
macro_rules! service_check {
    ($v: expr) => {
        $crate::service_check!($v => {});
    };
    ($v: expr => $e: expr) => {
        match $v {
            Ok(v) => v,
            Err(e) => {
                $crate::service_error!("{}", e);
                $e
            },
        }
    };
}

#[derive(Error, Debug)]
#[error("{0}")]
pub struct Error(String);

impl Error {
    pub fn new(s: &str) -> Error {
        Error(s.to_string())
    }

    pub fn from<E: std::error::Error>(e: E) -> Self {
        Self(e.to_string())
    }
}

impl<S: Service> MessageIdentity for S {
    const NAME: &'static str = S::NAME;
}

/// Represent a service. A service is a concurrent entity with its own lifecycle.
/// A service has a [`MessageIdentity`] given by its [`Self::NAME`] and is running on a [`Service::Context`].
/// The service is created using [`Self::new`] and executed by calling [`Self::run`].
///
/// Service are registered on either [`ServiceManager`] or [`TokioServiceManager`] which manage their lifecycle.
/// They usually communicate through message sent on a [`Messages`] layer.
///
/// One can think of a service the same way one think of services in service-based architecture with the difference
/// that here they are all part of the same program.
#[async_trait]
pub trait Service {
    const NAME: &'static str;
    type Context: Clone + Send;

    /// Returns a new service instance
    async fn new(context: Self::Context) -> Self;

    /// Runs the given service. Run should never return in general, except if there is
    /// an unrecoverable error in which case the manager will create a new instance and
    /// execute it.
    async fn run(self) -> Result<(), Error>;
}
