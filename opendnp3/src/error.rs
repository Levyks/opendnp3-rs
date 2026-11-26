//! Error types shared across the crate.

/// Errors produced while parsing octet sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseError {
    /// The buffer did not contain enough octets to complete the operation.
    InsufficientBytes,
    /// A length field specified a value that is not valid for the context.
    BadLength,
    /// A frame failed a CRC check.
    BadCrc,
    /// An octet contained a value that could not be converted to its enum type.
    UnknownValue(u8),
    /// A header parameter failed validation (function code, FCV/FCB, etc.).
    BadHeader,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InsufficientBytes => write!(f, "insufficient bytes"),
            Self::BadLength => write!(f, "bad length"),
            Self::BadCrc => write!(f, "bad CRC"),
            Self::UnknownValue(v) => write!(f, "unknown value: {v:#04X}"),
            Self::BadHeader => write!(f, "bad header"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Errors produced while serializing into a fixed-size buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WriteError {
    /// The destination buffer did not have enough remaining space.
    InsufficientSpace,
}

impl core::fmt::Display for WriteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InsufficientSpace => write!(f, "insufficient space"),
        }
    }
}

impl std::error::Error for WriteError {}
