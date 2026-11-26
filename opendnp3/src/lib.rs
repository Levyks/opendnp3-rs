//! A runtime-agnostic implementation of the DNP3 (IEEE 1815) protocol.
//!
//! This crate is a from-scratch Rust rewrite of the archived C++
//! [opendnp3](https://github.com/dnp3/opendnp3) library. The core contains no
//! I/O and no runtime dependencies: protocol state machines are driven
//! explicitly and communicate via events, so any executor (tokio, async-std,
//! embedded, tests) can host them. See the `opendnp3-tokio` crate for
//! batteries-included TCP/TLS/serial integration.
//!
//! # Architecture
//!
//! DNP3 is a three-layer protocol:
//!
//! - [`link`] — FT3 framing: `0x0564` frames, per-block CRC-16, primary /
//!   secondary link-layer services.
//! - [`transport`] — segmentation and reassembly of application fragments
//!   across link-layer frames (TPDUs with FIR/FIN/sequence).
//! - [`app`] — application layer: APDUs, function codes, object headers,
//!   variations, and measurements.
//! - [`master`], [`outstation`] — protocol roles built on the layers above.
//! - [`gen`] — DNP3 enumerations (function codes, qualifiers, group/variation
//!   numbers, quality flags, ...).
//! - [`clock`] — time abstraction; the core never touches real time.

pub mod app;
pub mod clock;
pub mod enums;
pub mod error;
pub mod link;
pub mod master;
pub mod message;
pub mod outstation;
pub mod transport;
pub mod util;

pub use message::Message;
