//! Measurement static/event variation enums and event generation mode.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EventMode {
    #[default]
    Detect,
    Force,
    Suppress,
    EventOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StaticBinaryVariation {
    Group1Var1,
    #[default]
    Group1Var2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EventBinaryVariation {
    #[default]
    Group2Var1,
    Group2Var2,
    Group2Var3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StaticDoubleBinaryVariation {
    Group3Var1,
    #[default]
    Group3Var2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EventDoubleBinaryVariation {
    #[default]
    Group4Var1,
    Group4Var2,
    Group4Var3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StaticBinaryOutputStatusVariation {
    #[default]
    Group10Var2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EventBinaryOutputStatusVariation {
    #[default]
    Group11Var1,
    Group11Var2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StaticCounterVariation {
    #[default]
    Group20Var1,
    Group20Var2,
    Group20Var5,
    Group20Var6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EventCounterVariation {
    #[default]
    Group22Var1,
    Group22Var2,
    Group22Var5,
    Group22Var6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StaticFrozenCounterVariation {
    #[default]
    Group21Var1,
    Group21Var2,
    Group21Var5,
    Group21Var6,
    Group21Var9,
    Group21Var10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EventFrozenCounterVariation {
    #[default]
    Group23Var1,
    Group23Var2,
    Group23Var5,
    Group23Var6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StaticAnalogVariation {
    #[default]
    Group30Var1,
    Group30Var2,
    Group30Var3,
    Group30Var4,
    Group30Var5,
    Group30Var6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EventAnalogVariation {
    #[default]
    Group32Var1,
    Group32Var2,
    Group32Var3,
    Group32Var4,
    Group32Var5,
    Group32Var6,
    Group32Var7,
    Group32Var8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StaticAnalogOutputStatusVariation {
    #[default]
    Group40Var1,
    Group40Var2,
    Group40Var3,
    Group40Var4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EventAnalogOutputStatusVariation {
    #[default]
    Group42Var1,
    Group42Var2,
    Group42Var3,
    Group42Var4,
    Group42Var5,
    Group42Var6,
    Group42Var7,
    Group42Var8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StaticOctetStringVariation {
    #[default]
    Group110Var0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EventOctetStringVariation {
    #[default]
    Group111Var0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StaticTimeAndIntervalVariation {
    #[default]
    Group50Var4,
}
