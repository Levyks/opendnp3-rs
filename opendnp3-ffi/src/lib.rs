//! BoltFFI bindings for [`opendnp3`] (Python, C#, Java, Swift, Kotlin, TypeScript).
//!
//! 100% Tokio-free: target languages implement [`Dnp3Stream`] using their own
//! native async I/O (`asyncio`, C# `Task`/`Socket`, Java `NIO`/`Netty`, etc.)
//! and poll [`Master::run`] / [`Outstation::run`] directly.

#![recursion_limit = "256"]

boltffi::scaffolding!();

pub mod callbacks;
pub mod master;
pub mod outstation;
pub mod pipe;
#[cfg(feature = "smol-tcp")]
pub mod tcp;
pub mod types;

pub use callbacks::{CommandHandler, Dnp3Stream, SoeHandler};
pub use master::{CommandSet, Master};
pub use outstation::{DatabaseConfig, Outstation};
pub use pipe::BytePipe;
#[cfg(feature = "smol-tcp")]
pub use tcp::TcpListener;
pub use types::*;
