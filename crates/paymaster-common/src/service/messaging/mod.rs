mod message;
pub use message::{AsMessage, MessageIdentity, MessageReceiver, MessageReceiverBuilder, Messages};

/// Convenience macros to declare a [`MessageIdentity`] which allow to send/receive
/// message using [`Messages`]
/// Example
/// ```rust
/// use paymaster_common::declare_message_identity;
///
/// pub struct Sender;
///
/// declare_message_identity!(Sender);
/// ```
#[macro_export]
macro_rules! declare_message_identity {
    ($id: ident) => {
        impl $crate::service::messaging::MessageIdentity for $id {
            const NAME: &'static str = stringify!($id);
        }
    };
}

/// Convenience macro to declare a type as a message type. More specifically
/// it implements [`AsMessage`]
/// Example
/// ```rust
///  use paymaster_common::as_message;
///
///  pub enum Message {
///     MyMessageA(A)
///  }   
///
///  pub struct A;
///  as_message!(Message::MyMessageA => A);
/// ```
#[macro_export]
macro_rules! as_message {
    ($m: ident :: $s: ident => $t: ty) => {
        impl From<$t> for $m {
            fn from(value: $t) -> Self {
                $m::$s(value)
            }
        }

        #[allow(unreachable_patterns)]
        impl $crate::service::messaging::AsMessage<$t> for $m {
            fn is_message(&self) -> bool {
                match self {
                    $m::$s(_) => true,
                    _ => false,
                }
            }

            fn into_message(self) -> Option<$t> {
                match self {
                    $m::$s(x) => Some(x),
                    _ => None,
                }
            }

            fn into_message_unchecked(self) -> $t {
                self.into_message().unwrap()
            }
        }
    };
}

/// Convenience to send message using a [`Messages`]. Sending a message this way does not
/// wait for the message to be received.
/// Example 1
/// ```rust
/// use paymaster_common::send_message;
/// use paymaster_common::service::messaging::Messages;
///
/// #[derive(Clone)]
/// pub struct Message;
///
/// let mut messages = Messages::<Message>::new();
/// // Usable only inside function where Self: MessageIdentity like Service
/// send_message!(messages => Message);
/// ```
/// Example 2
/// ```rust
/// use paymaster_common::{declare_message_identity, send_message};
/// use paymaster_common::service::messaging::Messages;
///
/// pub struct Sender;
///
/// declare_message_identity!(Sender);
///
/// #[derive(Clone)]
/// pub struct Message;
///
/// let mut messages = Messages::<Message>::new();
/// send_message!(from: Sender ; messages => Message);
/// ```
#[macro_export]
macro_rules! send_message {
    ($messages: expr => $message: expr) => {
        $messages.publish::<Self>($message).await;
    };
    (from: $from : ty ; $messages: expr => $message: expr) => {
        $messages.publish::<$from>($message).await;
    };
}

/// Convenience macros to receive a message using a [`MessageReceiver`]
/// Example
/// ```rust
/// use paymaster_common::{declare_message_identity, receive_message};///
///
/// use paymaster_common::service::messaging::Messages;
///
/// pub struct Sender;
/// declare_message_identity!(Sender);
///
/// pub struct Receiver;
/// declare_message_identity!(Receiver);
///
/// #[derive(Clone)]
/// pub struct Message;
///
/// let mut messages = Messages::<Message>::new();
/// let mut receiver = messages
///     .receiver::<Receiver>()
///     .subscribe_to::<Sender>()
///     .build()
///     .await;
///
/// let message = receive_message!(receiver);
/// ```
#[macro_export]
macro_rules! receive_message {
    ($messages: expr) => {
        $messages.receive().await
    };
}

#[cfg(test)]
mod tests {
    #[derive(Debug, Clone)]
    pub struct MessageA;

    #[derive(Debug, Clone)]
    pub struct MessageB;

    #[derive(Debug, Clone)]
    pub enum Message {
        A(MessageA),
        B(MessageB),
    }

    as_message!(Message::A => MessageA);
    as_message!(Message::B => MessageB);
}
