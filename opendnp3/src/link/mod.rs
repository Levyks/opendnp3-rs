//! DNP3 link layer (FT3): framing, CRCs, and the link-layer state machine.

pub mod addresses;
pub mod config;
pub mod constants;
pub mod crc;
pub mod frame;
pub mod header;
pub mod layer;
pub mod parser;

pub use addresses::Addresses;
pub use config::LinkConfig;
pub use constants::LinkBroadcastAddress;
pub use header::LinkHeader;
pub use layer::{LinkEvent, LinkLayer, LinkStatistics, TransportSegment};
pub use parser::{FrameParser, LinkHeaderFields, ParserStatistics, ReceivedFrame};
