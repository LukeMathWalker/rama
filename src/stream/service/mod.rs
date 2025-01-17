//! Rama services that operate directly on [`crate::stream::Stream`] types.
//!
//! Examples are services that can operate directly on a `TCP`, `TLS` or `UDP` stream.

mod echo;
pub use echo::EchoService;
