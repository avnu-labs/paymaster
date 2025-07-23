use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::Arc;

use log::{error, warn};
use tokio::sync::{mpsc, RwLock};

/// Defines how to convert a message structure into the general message structure
/// used by the crate. In general one use the macro [`as_message`] to implement
/// this trait
pub trait AsMessage<M>
where
    Self: Sized,
    Self: From<M>,
{
    fn is_message(&self) -> bool;
    fn into_message(self) -> Option<M>;
    fn into_message_unchecked(self) -> M;
}

/// Identity used to send and receive message. Entity listen to sender that have
/// an identity and send message using their identity.
///
/// Warning: Identity must be unique !
pub trait MessageIdentity {
    const NAME: &'static str;
}

/// Messaging layer for inter-thread communication. Allow concurrent entity to
/// pass data between each other in a safe and sound manner. Internally it uses
/// a Arc<RWLock<_>> in order to be passed to multiple threads and accessed
/// concurrently.
#[derive(Clone)]
pub struct Messages<T>(Arc<RwLock<MessagesInner<T>>>)
where
    T: Clone,
    T: Send + Sync;

impl<T> Default for Messages<T>
where
    T: Clone,
    T: Send + Sync,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Messages<T>
where
    T: Clone,
    T: Send + Sync,
{
    /// Creates a new messaging layer. All Sender/Receiver must register on the same layer
    /// to be able to pass message around. Multiple layer can be created to segregate communication
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(MessagesInner::default())))
    }

    /// Publish a new message using the given [`MessageIdentity`]
    pub async fn publish<S: MessageIdentity>(&self, message: T) {
        self.0.read().await.publish::<S>(message).await
    }

    /// Returns a builder to create a new [`MessageReceiver`] bound to the given [`MessageIdentity`]
    /// Receiver are used to subscribe to different sender.
    pub fn receiver<S: MessageIdentity>(&mut self) -> MessageReceiverBuilder<T> {
        MessageReceiverBuilder {
            from: S::NAME.to_string(),
            to: HashSet::new(),
            messages: self.clone(),
        }
    }
}

/// Internal representation of the messaging layer. Uses a map of multi-producer/single-consumer channel.
#[derive(Debug)]
struct MessagesInner<T>
where
    T: Clone,
    T: Send + Sync,
{
    registrations: HashMap<String, HashMap<String, mpsc::Sender<T>>>,
}

impl<T> MessagesInner<T>
where
    T: Clone,
    T: Send + Sync,
{
    /// Publish a message to all the listener of the given [`MessageIdentity`]. In the case
    /// where a channel is closed or full, the corresponding message is dropped.
    pub async fn publish<S: MessageIdentity>(&self, message: T) {
        let registrations = self.registrations.get(S::NAME);
        if let Some(registrations) = registrations {
            for r in registrations.values() {
                // avoid slow-receiver bottleneck
                if r.capacity() == 0 || r.is_closed() {
                    continue;
                }

                if let Err(e) = r.send(message.clone()).await {
                    error!("{}", e);
                }
            }
        };
    }
}

impl<T> Default for MessagesInner<T>
where
    T: Clone,
    T: Send + Sync,
{
    fn default() -> Self {
        MessagesInner {
            registrations: HashMap::default(),
        }
    }
}

/// Allow entity to receive messages. Internally it uses a multi-producer/single-consumer channel
pub struct MessageReceiver<T>(mpsc::Receiver<T>)
where
    T: Clone,
    T: Send + Sync;

impl<T> MessageReceiver<T>
where
    T: Clone,
    T: Send + Sync,
{
    /// Wait to receive a message on the channel
    pub async fn receive(&mut self) -> Option<T> {
        self.0.recv().await
    }

    pub async fn receive_all(&mut self) -> Vec<T> {
        self.receive_until(|_| false).await
    }

    pub async fn receive_until<F: Fn(&T) -> bool>(&mut self, condition: F) -> Vec<T> {
        let mut messages = Vec::with_capacity(self.0.len());
        while !self.0.is_empty() {
            if let Some(message) = self.receive().await {
                messages.push(message)
            }

            let Some(last) = messages.last() else { continue };
            if condition(last) {
                return messages;
            }
        }

        messages
    }
}

/// Builder to create a [`MessageReceiver`]
/// Example
/// ```rust
/// use paymaster_common::service::messaging::Messages;
///
/// let mut messages = Messages::new();
/// let mut receiver = messages
///     .receiver()
///     .subscribe_to::<A>()
///     .subscribe_to::<B>()
///     .build()
///     .await
/// ```
pub struct MessageReceiverBuilder<T>
where
    T: Clone,
    T: Send + Sync,
{
    from: String,
    to: HashSet<String>,
    messages: Messages<T>,
}

impl<T> MessageReceiverBuilder<T>
where
    T: Clone,
    T: Send + Sync,
{
    /// Subscribe to the given [`MessageIdentity`]
    pub fn subscribe_to<S: MessageIdentity>(mut self) -> Self {
        self.to.insert(S::NAME.to_string());
        self
    }

    /// Returns a [`MessageReceiver`] that will listen to all the [`MessageIdentity`] specified
    /// A warning will be emitted if the same identity is already listening to a given [`MessageIdentity`]
    /// but will override the registrations meaning the previous receiver will no longer receive the messages
    pub async fn build(self) -> MessageReceiver<T> {
        let (tx, rx) = mpsc::channel(1024);

        let mut messages = self.messages.0.write().await;
        for to in self.to {
            let registrations = messages.registrations.entry(to).or_default();
            if registrations.contains_key(&self.from) {
                warn!("listener already registered {}, overriding", self.from)
            }

            registrations.insert(self.from.clone(), tx.clone());
        }

        MessageReceiver(rx)
    }
}

#[cfg(test)]
mod tests {
    use crate::declare_message_identity;
    use crate::service::messaging::Messages;

    #[derive(Debug, Clone)]
    struct Message {}

    struct ServiceA;

    declare_message_identity!(ServiceA);

    struct ServiceB;

    declare_message_identity!(ServiceB);

    struct ServiceC;

    declare_message_identity!(ServiceC);

    #[tokio::test]
    #[allow(unused_variables)]
    async fn test_register_service() {
        let mut messages: Messages<Message> = Messages::new();

        let a = messages.receiver::<ServiceA>().subscribe_to::<ServiceB>().build().await;
        let b = messages.receiver::<ServiceA>().subscribe_to::<ServiceC>().build().await;
    }

    #[tokio::test]
    #[allow(unused_variables)]
    async fn test_allow_multiple_subscribe() {
        let mut messages: Messages<Message> = Messages::new();

        let a = messages.receiver::<ServiceA>().subscribe_to::<ServiceB>().build().await;
        let b = messages.receiver::<ServiceA>().subscribe_to::<ServiceB>().build().await;
    }
}
