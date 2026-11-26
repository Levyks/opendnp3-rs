//! Channel lifecycle states.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelState {
    Closed,
    Opening,
    Open,
    Shutdown,
}
