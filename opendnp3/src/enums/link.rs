//! Link layer enumerations.

use super::dnp_enum;

dnp_enum! {
    /// Link layer function code enumeration.
    ///
    /// Note: values include the PRM bit (0x40) for primary-to-secondary
    /// functions, matching the C++ `LinkFunction` enum.
    LinkFunction, Invalid;
    /// Primary -> secondary: reset link states
    PriResetLinkStates = 0x40,
    /// Primary -> secondary: test link states
    PriTestLinkStates = 0x42,
    /// Primary -> secondary: confirmed user data
    PriConfirmedUserData = 0x43,
    /// Primary -> secondary: unconfirmed user data
    PriUnconfirmedUserData = 0x44,
    /// Primary -> secondary: request link status
    PriRequestLinkStatus = 0x49,
    /// Secondary -> primary: ACK
    SecAck = 0x00,
    /// Secondary -> primary: NACK
    SecNack = 0x01,
    /// Secondary -> primary: link status
    SecLinkStatus = 0x0B,
    /// Secondary -> primary: function not supported
    SecNotSupported = 0x0F,
}

impl LinkFunction {
    /// Human-readable name for logging.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PriResetLinkStates => "PRI_RESET_LINK_STATES",
            Self::PriTestLinkStates => "PRI_TEST_LINK_STATES",
            Self::PriConfirmedUserData => "PRI_CONFIRMED_USER_DATA",
            Self::PriUnconfirmedUserData => "PRI_UNCONFIRMED_USER_DATA",
            Self::PriRequestLinkStatus => "PRI_REQUEST_LINK_STATUS",
            Self::SecAck => "SEC_ACK",
            Self::SecNack => "SEC_NACK",
            Self::SecLinkStatus => "SEC_LINK_STATUS",
            Self::SecNotSupported => "SEC_NOT_SUPPORTED",
            Self::Invalid => "INVALID",
        }
    }
}

impl core::fmt::Display for LinkFunction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

dnp_enum! {
    /// Link status as reported by an outstation / channel.
    LinkStatus, Unknown;
    /// Link layer is unreset (need reset link states)
    Unreset = 0,
    /// Link layer is reset and available for confirmed user data
    Reset = 1,
}
