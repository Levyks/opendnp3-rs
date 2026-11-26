//! BoltFFI bindings for [`opendnp3`] (Python, C#, Java, Swift, Kotlin, TypeScript).
//!
//! 100% Tokio-free: target languages implement [`Dnp3Stream`] using their own
//! native async I/O (`asyncio`, C# `Task`/`Socket`, Java `NIO`/`Netty`, etc.)
//! and poll [`FfiMaster::run`] / [`FfiOutstation::run`] directly.

#![recursion_limit = "256"]

boltffi::scaffolding!();

pub mod callbacks;
pub mod master;
pub mod outstation;
pub mod pipe;
pub mod types;

pub use callbacks::{Dnp3Stream, FfiCommandHandler, FfiSoeHandler};
pub use master::{FfiCommandSet, FfiMaster};
pub use outstation::{FfiDatabaseConfig, FfiOutstation};
pub use pipe::FfiBytePipe;
pub use types::*;
