use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use log::{error, info};
use tokio::task::JoinSet;
use tokio::time;

use crate::service::{Error, Service};

/// Service manager used to spawn [`Service`] and manage their lifecycle.
/// This manager used thread from the standard library to spawn services
/// and internally initialize a Tokio runtime.
///
/// The difference between [`ServiceManager`] and [`TokioServiceManager`] is that
/// each service has its own Tokio runtime which means that task spawned by services
/// are segregated. On the other hand, the service spawn by the latter are all running
/// on the same Tokio Runtime and the task they will spawn will also share that same Runtime
pub struct ServiceManager<C> {
    context: C,

    services: Vec<JoinHandle<()>>,
}

impl<C> ServiceManager<C>
where
    C: 'static + Clone + Send,
{
    /// Create a new manager on the given context. The context will be cloned and passed
    /// to each service
    pub fn new(context: C) -> Self {
        Self { context, services: vec![] }
    }

    /// Spawn a new service on the manager, giving it the bound context. Service will be restarted in
    /// case they throw an error.
    pub fn spawn<T: Service<Context = C>>(&mut self) {
        let ctx = self.context.clone();

        self.services.push(thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();

            runtime.block_on(async move {
                loop {
                    let service = T::new(ctx.clone()).await;

                    info!(target: T::NAME , "starting service");
                    if let Err(err) = service.run().await {
                        error!(target: T::NAME , "service terminated with error {} - restarting in 5sec", err);
                        time::sleep(Duration::from_secs(5)).await;
                    }
                }
            })
        }))
    }

    /// Convenience method to spawn a service only if a condition is met.
    pub fn spawn_conditional<T: Service<Context = C>>(&mut self, condition: bool) {
        if condition {
            self.spawn::<T>()
        }
    }

    /// Pause the current thread and let the service ran
    pub fn wait(&mut self) -> Result<(), Error> {
        if let Some(service) = self.services.pop() {
            let _ = service.join();
            return Err(Error::new("service manager error"));
        }

        Ok(())
    }
}

/// Service manager used to spawn [`Service`] and manage their lifecycle.
/// This manager uses the same Tokio Runtime for all service spawned. See
/// [`ServiceManager`] for more information
pub struct TokioServiceManager<C> {
    context: C,

    services: JoinSet<()>,
}

impl<C> TokioServiceManager<C>
where
    C: 'static + Clone + Send,
{
    /// Create a new manager on the given context. The context will be cloned and passed
    /// to each service
    pub fn new(context: C) -> Self {
        Self {
            context,
            services: JoinSet::new(),
        }
    }

    /// Spawn a new service on the manager, giving it the bound context. Service will be restarted in
    /// case they throw an error.
    pub fn spawn<T: Service<Context = C>>(&mut self) {
        let ctx = self.context.clone();

        self.services.spawn(async move {
            loop {
                let service = T::new(ctx.clone()).await;

                info!(target: T::NAME , "starting service");
                if let Err(err) = service.run().await {
                    error!(target: T::NAME , "service terminated with error {} - restarting in 5sec", err);
                    time::sleep(Duration::from_secs(5)).await;
                }
            }
        });
    }

    /// Convenience method to spawn a service only if a condition is met.
    pub fn spawn_conditional<T: Service<Context = C>>(&mut self, condition: bool) {
        if condition {
            self.spawn::<T>()
        }
    }

    /// Pause the current thread and let the service ran
    pub async fn wait(&mut self) -> Result<(), Error> {
        if self.services.join_next().await.is_some() {
            return Err(Error::new("service manager error"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use async_trait::async_trait;

    use crate::service::messaging::Messages;
    use crate::service::{Error, Service, ServiceManager, TokioServiceManager};
    use crate::{declare_message_identity, receive_message, send_message};

    #[derive(Clone)]
    struct Context(Messages<Message>);

    #[derive(Clone, Hash, PartialEq, Eq)]
    enum Message {
        A,
        B,
    }

    struct Tester;

    declare_message_identity!(Tester);

    struct ServiceA(Context);

    #[async_trait]
    impl Service for ServiceA {
        type Context = Context;

        const NAME: &'static str = "ServiceA";

        async fn new(context: Self::Context) -> Self {
            Self(context)
        }

        async fn run(self) -> Result<(), Error> {
            send_message!(self.0.0 => Message::A);

            Ok(())
        }
    }

    struct ServiceB(Context);

    #[async_trait]
    impl Service for ServiceB {
        type Context = Context;

        const NAME: &'static str = "ServiceB";

        async fn new(context: Self::Context) -> Self {
            Self(context)
        }

        async fn run(self) -> Result<(), Error> {
            send_message!(self.0.0 => Message::B);

            Err(Error::new("dummy"))
        }
    }

    #[tokio::test]
    async fn service_manager_run_properly() {
        let mut messages = Messages::new();
        let mut receiver = messages
            .receiver::<Tester>()
            .subscribe_to::<ServiceA>()
            .subscribe_to::<ServiceB>()
            .build()
            .await;

        let context = Context(messages);
        let mut services = ServiceManager::new(context);
        services.spawn::<ServiceA>();
        services.spawn::<ServiceB>();

        let mut received_messages = HashSet::new();
        loop {
            received_messages.insert(receive_message!(receiver));
            if received_messages.len() == 2 {
                break;
            }
        }
    }

    #[tokio::test]
    async fn light_service_manager_run_properly() {
        let mut messages = Messages::new();
        let mut receiver = messages
            .receiver::<Tester>()
            .subscribe_to::<ServiceA>()
            .subscribe_to::<ServiceB>()
            .build()
            .await;

        let context = Context(messages);
        let mut services = TokioServiceManager::new(context);
        services.spawn::<ServiceA>();
        services.spawn::<ServiceB>();

        let mut received_messages = HashSet::new();
        loop {
            received_messages.insert(receive_message!(receiver));
            if received_messages.len() == 2 {
                break;
            }
        }
    }

    #[tokio::test]
    async fn service_manager_restart_service_properly() {
        let mut messages = Messages::new();
        let mut receiver = messages.receiver::<Tester>().subscribe_to::<ServiceB>().build().await;

        let context = Context(messages);
        let mut services = ServiceManager::new(context);
        services.spawn::<ServiceB>();

        let mut received_messages = Vec::new();
        loop {
            received_messages.push(receive_message!(receiver));
            if received_messages.len() == 2 {
                break;
            }
        }
    }

    #[tokio::test]
    async fn light_service_manager_restart_service_properly() {
        let mut messages = Messages::new();
        let mut receiver = messages.receiver::<Tester>().subscribe_to::<ServiceB>().build().await;

        let context = Context(messages);
        let mut services = TokioServiceManager::new(context);
        services.spawn::<ServiceB>();

        let mut received_messages = Vec::new();
        loop {
            received_messages.push(receive_message!(receiver));
            if received_messages.len() == 2 {
                break;
            }
        }
    }
}
