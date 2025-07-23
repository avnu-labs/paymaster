/// Convenience macros to dispatch enum methods
/// Example
/// ```rust
/// use paymaster_common::enum_dispatch;
///
/// pub struct Foo {
///     A(MyA),
///     B(MyB)
/// }
///
/// impl Foo {
///    pub fn bar(&self) {
///       match self {
///          Self::A(x) => x.bar(),
///          Self::B(x) => x.bar()
///       }
///       // Equivalent to
///       enum_dispatch!(self {
///          Self::A(x) |
///          Self::B(x) => x.bar()
///       })
///    }
/// }
///
/// ```
#[macro_export]
macro_rules! enum_dispatch {
    ($self: ident { $($($variant: pat_param)|* => $do: expr),+ }) => {
        match $self {
            $(
                $($variant => $do),+
            ),+
        }
    };
}
