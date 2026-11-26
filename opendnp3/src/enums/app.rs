//! Application-layer DNP3 enumerations (IEEE 1815).

use std::fmt;

use super::dnp_enum;

dnp_enum! {
    /// Application-layer function code (octet 1 of every APDU).
    FunctionCode, Unknown;
    Confirm = 0x00,
    Read = 0x01,
    Write = 0x02,
    Select = 0x03,
    Operate = 0x04,
    DirectOperate = 0x05,
    DirectOperateNr = 0x06,
    ImmedFreeze = 0x07,
    ImmedFreezeNr = 0x08,
    FreezeClear = 0x09,
    FreezeClearNr = 0x0A,
    FreezeAtTime = 0x0B,
    FreezeAtTimeNr = 0x0C,
    ColdRestart = 0x0D,
    WarmRestart = 0x0E,
    InitializeData = 0x0F,
    InitializeApplication = 0x10,
    StartApplication = 0x11,
    StopApplication = 0x12,
    SaveConfiguration = 0x13,
    EnableUnsolicited = 0x14,
    DisableUnsolicited = 0x15,
    AssignClass = 0x16,
    DelayMeasure = 0x17,
    RecordCurrentTime = 0x18,
    OpenFile = 0x19,
    CloseFile = 0x1A,
    DeleteFile = 0x1B,
    GetFileInfo = 0x1C,
    AuthenticateFile = 0x1D,
    AbortFile = 0x1E,
    Response = 0x81,
    UnsolicitedResponse = 0x82,
}

impl fmt::Display for FunctionCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

dnp_enum! {
    /// Object header qualifier code (index prefix + range specifier).
    QualifierCode, Undefined;
    Uint8StartStop = 0x00,
    Uint16StartStop = 0x01,
    AllObjects = 0x06,
    Uint8Cnt = 0x07,
    Uint16Cnt = 0x08,
    Uint8CntUint8Index = 0x17,
    Uint16CntUint16Index = 0x28,
    Uint16FreeFormat = 0x5B,
}

dnp_enum! {
    /// Command status returned in CROB and Analog Output responses.
    CommandStatus, Undefined;
    Success = 0,
    Timeout = 1,
    NoSelect = 2,
    FormatError = 3,
    NotSupported = 4,
    AlreadyActive = 5,
    HardwareError = 6,
    Local = 7,
    TooManyOps = 8,
    NotAuthorized = 9,
    AutomationInhibit = 10,
    ProcessingLimited = 11,
    OutOfRange = 12,
    DownstreamLocal = 13,
    AlreadyComplete = 14,
    Blocked = 15,
    Cancelled = 16,
    BlockedOtherMaster = 17,
    DownstreamFail = 18,
    NonParticipating = 126,
}

dnp_enum! {
    /// Operation type field (bits 0..=3) of a CROB control code.
    OperationType, Undefined;
    Nul = 0,
    PulseOn = 1,
    PulseOff = 2,
    LatchOn = 3,
    LatchOff = 4,
}

dnp_enum! {
    /// Trip-Close code field (bits 6..=7) of a CROB control code.
    TripCloseCode, Reserved;
    Nul = 0,
    Close = 1,
    Trip = 2,
}

/// How an outstation command is being operated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperateType {
    SelectBeforeOperate,
    DirectOperate,
    DirectOperateNoAck,
}

/// Point class bitmask values (Class 0..3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PointClass {
    Class0 = 0x01,
    Class1 = 0x02,
    Class2 = 0x04,
    Class3 = 0x08,
}

/// Event class (1, 2, or 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventClass {
    Ec1 = 0,
    Ec2 = 1,
    Ec3 = 2,
}

impl EventClass {
    pub const fn to_point_class(self) -> PointClass {
        match self {
            Self::Ec1 => PointClass::Class1,
            Self::Ec2 => PointClass::Class2,
            Self::Ec3 => PointClass::Class3,
        }
    }
}

/// Timestamp quality attached to measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TimestampQuality {
    #[default]
    Invalid = 0,
    Synchronized = 1,
    Unsynchronized = 2,
}

/// Cold or warm restart type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RestartType {
    Cold,
    Warm,
}

/// Outstation restart support mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RestartMode {
    Unsupported,
    SupportedDelayFine,
    SupportedDelayCoarse,
}

/// Group + Variation identifier packed as `(group << 8) | variation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum GroupVariation {
    Group1Var0 = 0x0100,
    Group1Var1 = 0x0101,
    Group1Var2 = 0x0102,

    Group2Var0 = 0x0200,
    Group2Var1 = 0x0201,
    Group2Var2 = 0x0202,
    Group2Var3 = 0x0203,

    Group3Var0 = 0x0300,
    Group3Var1 = 0x0301,
    Group3Var2 = 0x0302,

    Group4Var0 = 0x0400,
    Group4Var1 = 0x0401,
    Group4Var2 = 0x0402,
    Group4Var3 = 0x0403,

    Group10Var0 = 0x0A00,
    Group10Var1 = 0x0A01,
    Group10Var2 = 0x0A02,

    Group11Var0 = 0x0B00,
    Group11Var1 = 0x0B01,
    Group11Var2 = 0x0B02,

    Group12Var0 = 0x0C00,
    Group12Var1 = 0x0C01,

    Group13Var1 = 0x0D01,
    Group13Var2 = 0x0D02,

    Group20Var0 = 0x1400,
    Group20Var1 = 0x1401,
    Group20Var2 = 0x1402,
    Group20Var5 = 0x1405,
    Group20Var6 = 0x1406,

    Group21Var0 = 0x1500,
    Group21Var1 = 0x1501,
    Group21Var2 = 0x1502,
    Group21Var5 = 0x1505,
    Group21Var6 = 0x1506,
    Group21Var9 = 0x1509,
    Group21Var10 = 0x150A,

    Group22Var0 = 0x1600,
    Group22Var1 = 0x1601,
    Group22Var2 = 0x1602,
    Group22Var5 = 0x1605,
    Group22Var6 = 0x1606,

    Group23Var0 = 0x1700,
    Group23Var1 = 0x1701,
    Group23Var2 = 0x1702,
    Group23Var5 = 0x1705,
    Group23Var6 = 0x1706,

    Group30Var0 = 0x1E00,
    Group30Var1 = 0x1E01,
    Group30Var2 = 0x1E02,
    Group30Var3 = 0x1E03,
    Group30Var4 = 0x1E04,
    Group30Var5 = 0x1E05,
    Group30Var6 = 0x1E06,

    Group32Var0 = 0x2000,
    Group32Var1 = 0x2001,
    Group32Var2 = 0x2002,
    Group32Var3 = 0x2003,
    Group32Var4 = 0x2004,
    Group32Var5 = 0x2005,
    Group32Var6 = 0x2006,
    Group32Var7 = 0x2007,
    Group32Var8 = 0x2008,

    Group40Var0 = 0x2800,
    Group40Var1 = 0x2801,
    Group40Var2 = 0x2802,
    Group40Var3 = 0x2803,
    Group40Var4 = 0x2804,

    Group41Var0 = 0x2900,
    Group41Var1 = 0x2901,
    Group41Var2 = 0x2902,
    Group41Var3 = 0x2903,
    Group41Var4 = 0x2904,

    Group42Var0 = 0x2A00,
    Group42Var1 = 0x2A01,
    Group42Var2 = 0x2A02,
    Group42Var3 = 0x2A03,
    Group42Var4 = 0x2A04,
    Group42Var5 = 0x2A05,
    Group42Var6 = 0x2A06,
    Group42Var7 = 0x2A07,
    Group42Var8 = 0x2A08,

    Group43Var1 = 0x2B01,
    Group43Var2 = 0x2B02,
    Group43Var3 = 0x2B03,
    Group43Var4 = 0x2B04,
    Group43Var5 = 0x2B05,
    Group43Var6 = 0x2B06,
    Group43Var7 = 0x2B07,
    Group43Var8 = 0x2B08,

    Group50Var1 = 0x3201,
    Group50Var3 = 0x3203,
    Group50Var4 = 0x3204,

    Group51Var1 = 0x3301,
    Group51Var2 = 0x3302,

    Group52Var1 = 0x3401,
    Group52Var2 = 0x3402,

    Group60Var1 = 0x3C01,
    Group60Var2 = 0x3C02,
    Group60Var3 = 0x3C03,
    Group60Var4 = 0x3C04,

    Group70Var1 = 0x4601,
    Group70Var2 = 0x4602,
    Group70Var3 = 0x4603,
    Group70Var4 = 0x4604,
    Group70Var5 = 0x4605,
    Group70Var6 = 0x4606,
    Group70Var7 = 0x4607,
    Group70Var8 = 0x4608,

    Group80Var1 = 0x5001,

    Group110Var0 = 0x6E00,
    Group111Var0 = 0x6F00,

    Unknown = 0xFFFF,
}

impl GroupVariation {
    pub const fn to_u16(self) -> u16 {
        self as u16
    }

    pub const fn group(self) -> u8 {
        ((self as u16) >> 8) as u8
    }

    pub const fn variation(self) -> u8 {
        ((self as u16) & 0xFF) as u8
    }

    pub const fn from_group_var(group: u8, variation: u8) -> Self {
        Self::from_u16(((group as u16) << 8) | (variation as u16))
    }

    pub const fn from_u16(value: u16) -> Self {
        match value {
            0x0100 => Self::Group1Var0,
            0x0101 => Self::Group1Var1,
            0x0102 => Self::Group1Var2,
            0x0200 => Self::Group2Var0,
            0x0201 => Self::Group2Var1,
            0x0202 => Self::Group2Var2,
            0x0203 => Self::Group2Var3,
            0x0300 => Self::Group3Var0,
            0x0301 => Self::Group3Var1,
            0x0302 => Self::Group3Var2,
            0x0400 => Self::Group4Var0,
            0x0401 => Self::Group4Var1,
            0x0402 => Self::Group4Var2,
            0x0403 => Self::Group4Var3,
            0x0A00 => Self::Group10Var0,
            0x0A01 => Self::Group10Var1,
            0x0A02 => Self::Group10Var2,
            0x0B00 => Self::Group11Var0,
            0x0B01 => Self::Group11Var1,
            0x0B02 => Self::Group11Var2,
            0x0C00 => Self::Group12Var0,
            0x0C01 => Self::Group12Var1,
            0x0D01 => Self::Group13Var1,
            0x0D02 => Self::Group13Var2,
            0x1400 => Self::Group20Var0,
            0x1401 => Self::Group20Var1,
            0x1402 => Self::Group20Var2,
            0x1405 => Self::Group20Var5,
            0x1406 => Self::Group20Var6,
            0x1500 => Self::Group21Var0,
            0x1501 => Self::Group21Var1,
            0x1502 => Self::Group21Var2,
            0x1505 => Self::Group21Var5,
            0x1506 => Self::Group21Var6,
            0x1509 => Self::Group21Var9,
            0x150A => Self::Group21Var10,
            0x1600 => Self::Group22Var0,
            0x1601 => Self::Group22Var1,
            0x1602 => Self::Group22Var2,
            0x1605 => Self::Group22Var5,
            0x1606 => Self::Group22Var6,
            0x1700 => Self::Group23Var0,
            0x1701 => Self::Group23Var1,
            0x1702 => Self::Group23Var2,
            0x1705 => Self::Group23Var5,
            0x1706 => Self::Group23Var6,
            0x1E00 => Self::Group30Var0,
            0x1E01 => Self::Group30Var1,
            0x1E02 => Self::Group30Var2,
            0x1E03 => Self::Group30Var3,
            0x1E04 => Self::Group30Var4,
            0x1E05 => Self::Group30Var5,
            0x1E06 => Self::Group30Var6,
            0x2000 => Self::Group32Var0,
            0x2001 => Self::Group32Var1,
            0x2002 => Self::Group32Var2,
            0x2003 => Self::Group32Var3,
            0x2004 => Self::Group32Var4,
            0x2005 => Self::Group32Var5,
            0x2006 => Self::Group32Var6,
            0x2007 => Self::Group32Var7,
            0x2008 => Self::Group32Var8,
            0x2800 => Self::Group40Var0,
            0x2801 => Self::Group40Var1,
            0x2802 => Self::Group40Var2,
            0x2803 => Self::Group40Var3,
            0x2804 => Self::Group40Var4,
            0x2900 => Self::Group41Var0,
            0x2901 => Self::Group41Var1,
            0x2902 => Self::Group41Var2,
            0x2903 => Self::Group41Var3,
            0x2904 => Self::Group41Var4,
            0x2A00 => Self::Group42Var0,
            0x2A01 => Self::Group42Var1,
            0x2A02 => Self::Group42Var2,
            0x2A03 => Self::Group42Var3,
            0x2A04 => Self::Group42Var4,
            0x2A05 => Self::Group42Var5,
            0x2A06 => Self::Group42Var6,
            0x2A07 => Self::Group42Var7,
            0x2A08 => Self::Group42Var8,
            0x2B01 => Self::Group43Var1,
            0x2B02 => Self::Group43Var2,
            0x2B03 => Self::Group43Var3,
            0x2B04 => Self::Group43Var4,
            0x2B05 => Self::Group43Var5,
            0x2B06 => Self::Group43Var6,
            0x2B07 => Self::Group43Var7,
            0x2B08 => Self::Group43Var8,
            0x3201 => Self::Group50Var1,
            0x3203 => Self::Group50Var3,
            0x3204 => Self::Group50Var4,
            0x3301 => Self::Group51Var1,
            0x3302 => Self::Group51Var2,
            0x3401 => Self::Group52Var1,
            0x3402 => Self::Group52Var2,
            0x3C01 => Self::Group60Var1,
            0x3C02 => Self::Group60Var2,
            0x3C03 => Self::Group60Var3,
            0x3C04 => Self::Group60Var4,
            0x4601 => Self::Group70Var1,
            0x4602 => Self::Group70Var2,
            0x4603 => Self::Group70Var3,
            0x4604 => Self::Group70Var4,
            0x4605 => Self::Group70Var5,
            0x4606 => Self::Group70Var6,
            0x4607 => Self::Group70Var7,
            0x4608 => Self::Group70Var8,
            0x5001 => Self::Group80Var1,
            0x6E00 => Self::Group110Var0,
            0x6F00 => Self::Group111Var0,
            _ => Self::Unknown,
        }
    }
}
