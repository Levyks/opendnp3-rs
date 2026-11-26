//! A runtime-agnostic, `no_std + alloc` implementation of the DNP3 (IEEE 1815) protocol.
//!
//! This crate is a from-scratch Rust rewrite of the archived C++
//! [opendnp3](https://github.com/dnp3/opendnp3) library. The core contains no
//! I/O and no runtime dependencies: protocol state machines are driven
//! explicitly and communicate via events, so any executor (Tokio, smol,
//! Embassy, compio, blocking std, or foreign languages via BoltFFI) can host
//! them.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod app;
pub mod clock;
#[cfg(feature = "async")]
pub mod driver;
pub mod enums;
pub mod error;
pub mod link;
pub mod master;
pub mod message;
pub mod outstation;
pub mod transport;
pub mod util;

pub use message::Message;
