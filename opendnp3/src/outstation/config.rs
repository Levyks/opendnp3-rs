//! Outstation configuration structs.

use alloc::collections::BTreeMap;
use core::time::Duration;

use crate::app::{ClassField, DEFAULT_MAX_APDU_SIZE};
use crate::enums::{
    EventAnalogOutputStatusVariation, EventAnalogVariation, EventBinaryOutputStatusVariation,
    EventBinaryVariation, EventClass, EventCounterVariation, EventDoubleBinaryVariation,
    EventFrozenCounterVariation, EventOctetStringVariation, StaticAnalogOutputStatusVariation,
    StaticAnalogVariation, StaticBinaryOutputStatusVariation, StaticBinaryVariation,
    StaticCounterVariation, StaticDoubleBinaryVariation, StaticFrozenCounterVariation,
    StaticOctetStringVariation, StaticTimeAndIntervalVariation,
};

/// Operational parameters for an outstation.
#[derive(Debug, Clone)]
pub struct OutstationParams {
    pub max_tx_frag_size: usize,
    pub max_rx_frag_size: usize,
    pub allow_unsolicited: bool,
    pub sol_confirm_timeout: Duration,
    pub unsol_confirm_timeout: Duration,
    pub unsol_retry_delay: Duration,
    pub select_timeout: Duration,
    pub max_controls_per_request: usize,
    pub unsol_class_mask: ClassField,
    pub respond_to_any_master: bool,
}

impl Default for OutstationParams {
    fn default() -> Self {
        Self {
            max_tx_frag_size: DEFAULT_MAX_APDU_SIZE,
            max_rx_frag_size: DEFAULT_MAX_APDU_SIZE,
            allow_unsolicited: false,
            sol_confirm_timeout: Duration::from_secs(5),
            unsol_confirm_timeout: Duration::from_secs(5),
            unsol_retry_delay: Duration::from_secs(5),
            select_timeout: Duration::from_secs(10),
            max_controls_per_request: 16,
            unsol_class_mask: ClassField::ALL_EVENT_CLASSES,
            respond_to_any_master: false,
        }
    }
}

/// Per-type capacity limits for the outstation event buffer.
#[derive(Debug, Clone, Copy)]
pub struct EventBufferConfig {
    pub max_binary_events: usize,
    pub max_double_binary_events: usize,
    pub max_binary_output_status_events: usize,
    pub max_counter_events: usize,
    pub max_frozen_counter_events: usize,
    pub max_analog_events: usize,
    pub max_analog_output_status_events: usize,
    pub max_octet_string_events: usize,
}

impl EventBufferConfig {
    pub const fn all_types(size: usize) -> Self {
        Self {
            max_binary_events: size,
            max_double_binary_events: size,
            max_binary_output_status_events: size,
            max_counter_events: size,
            max_frozen_counter_events: size,
            max_analog_events: size,
            max_analog_output_status_events: size,
            max_octet_string_events: size,
        }
    }

    pub const fn total_capacity(&self) -> usize {
        self.max_binary_events
            + self.max_double_binary_events
            + self.max_binary_output_status_events
            + self.max_counter_events
            + self.max_frozen_counter_events
            + self.max_analog_events
            + self.max_analog_output_status_events
            + self.max_octet_string_events
    }
}

impl Default for EventBufferConfig {
    fn default() -> Self {
        Self::all_types(10)
    }
}

/// Per-point configuration for a static/event point.
#[derive(Debug, Clone, Copy)]
pub struct PointConfig<S, E> {
    pub class: Option<EventClass>,
    pub s_var: S,
    pub e_var: E,
    pub deadband: f64,
}

impl<S: Default, E: Default> Default for PointConfig<S, E> {
    fn default() -> Self {
        Self {
            class: Some(EventClass::Ec1),
            s_var: S::default(),
            e_var: E::default(),
            deadband: 0.0,
        }
    }
}

pub type BinaryConfig = PointConfig<StaticBinaryVariation, EventBinaryVariation>;
pub type DoubleBinaryConfig = PointConfig<StaticDoubleBinaryVariation, EventDoubleBinaryVariation>;
pub type BinaryOutputStatusConfig =
    PointConfig<StaticBinaryOutputStatusVariation, EventBinaryOutputStatusVariation>;
pub type CounterConfig = PointConfig<StaticCounterVariation, EventCounterVariation>;
pub type FrozenCounterConfig =
    PointConfig<StaticFrozenCounterVariation, EventFrozenCounterVariation>;
pub type AnalogConfig = PointConfig<StaticAnalogVariation, EventAnalogVariation>;
pub type AnalogOutputStatusConfig =
    PointConfig<StaticAnalogOutputStatusVariation, EventAnalogOutputStatusVariation>;
pub type OctetStringConfig = PointConfig<StaticOctetStringVariation, EventOctetStringVariation>;

/// Outstation static database layout configuration.
#[derive(Debug, Clone, Default)]
pub struct DatabaseConfig {
    pub binary: BTreeMap<u16, BinaryConfig>,
    pub double_binary: BTreeMap<u16, DoubleBinaryConfig>,
    pub binary_output_status: BTreeMap<u16, BinaryOutputStatusConfig>,
    pub counter: BTreeMap<u16, CounterConfig>,
    pub frozen_counter: BTreeMap<u16, FrozenCounterConfig>,
    pub analog: BTreeMap<u16, AnalogConfig>,
    pub analog_output_status: BTreeMap<u16, AnalogOutputStatusConfig>,
    pub octet_string: BTreeMap<u16, OctetStringConfig>,
    pub time_and_interval: BTreeMap<u16, StaticTimeAndIntervalVariation>,
}

impl DatabaseConfig {
    /// Create a database with `count` points (`0..count`) of every measurement type.
    pub fn all_types(count: u16) -> Self {
        let mut cfg = Self::default();
        for i in 0..count {
            cfg.binary.insert(i, BinaryConfig::default());
            cfg.double_binary.insert(i, DoubleBinaryConfig::default());
            cfg.binary_output_status
                .insert(i, BinaryOutputStatusConfig::default());
            cfg.counter.insert(i, CounterConfig::default());
            cfg.frozen_counter.insert(i, FrozenCounterConfig::default());
            cfg.analog.insert(i, AnalogConfig::default());
            cfg.analog_output_status
                .insert(i, AnalogOutputStatusConfig::default());
            cfg.octet_string.insert(i, OctetStringConfig::default());
            cfg.time_and_interval
                .insert(i, StaticTimeAndIntervalVariation::default());
        }
        cfg
    }
}

/// Complete outstation configuration.
#[derive(Debug, Clone, Default)]
pub struct OutstationConfig {
    pub params: OutstationParams,
    pub event_buffer: EventBufferConfig,
    pub database: DatabaseConfig,
}
