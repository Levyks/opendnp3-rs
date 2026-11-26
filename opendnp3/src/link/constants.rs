//! Link-layer (FT3) constants.

/// Minimum value of the LENGTH field (header only, no user data).
pub const LPDU_MIN_LENGTH: u8 = 5;
/// Maximum value of the LENGTH field.
pub const LPDU_MAX_LENGTH: u8 = 255;
/// Total header size including start bytes, length, control, addresses, CRC.
pub const LPDU_HEADER_SIZE: usize = 10;
/// User-data block size; a CRC follows every block.
pub const LPDU_DATA_BLOCK_SIZE: usize = 16;
/// CRC size in bytes.
pub const LPDU_CRC_SIZE: usize = 2;
/// One full data block plus its CRC.
pub const LPDU_DATA_PLUS_CRC_SIZE: usize = 18;
/// Maximum user-data payload in a single frame.
pub const LPDU_MAX_USER_DATA_SIZE: usize = 250;
/// Maximum total frame size: 10 (header) + 250 (user data) + 32 (block CRCs).
pub const LPDU_MAX_FRAME_SIZE: usize = 292;

/// Broadcast addresses (destination field).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum LinkBroadcastAddress {
    DontConfirm = 0xFFFD,
    ShallConfirm = 0xFFFE,
    OptionalConfirm = 0xFFFF,
}

impl LinkBroadcastAddress {
    pub const fn to_u16(self) -> u16 {
        self as u16
    }

    /// Is `addr` one of the broadcast addresses?
    pub fn is_broadcast(addr: u16) -> bool {
        matches!(addr, 0xFFFD..=0xFFFF)
    }

    pub fn try_from_u16(addr: u16) -> Option<Self> {
        match addr {
            0xFFFD => Some(Self::DontConfirm),
            0xFFFE => Some(Self::ShallConfirm),
            0xFFFF => Some(Self::OptionalConfirm),
            _ => None,
        }
    }
}

// Header field indices
pub(crate) const LI_LENGTH: usize = 2;
pub(crate) const LI_CONTROL: usize = 3;
pub(crate) const LI_DESTINATION: usize = 4;
pub(crate) const LI_SOURCE: usize = 6;
pub(crate) const LI_CRC: usize = 8;

// CONTROL byte masks
pub(crate) const MASK_DIR: u8 = 0x80;
pub(crate) const MASK_PRM: u8 = 0x40;
pub(crate) const MASK_FCB: u8 = 0x20;
pub(crate) const MASK_FCV: u8 = 0x10;
pub(crate) const MASK_FUNC_OR_PRM: u8 = MASK_PRM | 0x0F;
