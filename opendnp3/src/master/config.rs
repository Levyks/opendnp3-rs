//! Master configuration parameters.

use core::time::Duration;

use crate::app::{ClassField, DEFAULT_MAX_APDU_SIZE};
use crate::enums::TimeSyncMode;

/// Operational parameters for a DNP3 master.
#[derive(Debug, Clone)]
pub struct MasterParams {
    pub response_timeout: Duration,
    pub time_sync_mode: TimeSyncMode,
    pub disable_unsol_on_startup: bool,
    pub ignore_restart_iin: bool,
    pub unsol_class_mask: ClassField,
    pub startup_integrity_class_mask: ClassField,
    pub integrity_on_event_overflow_iin: bool,
    pub event_scan_on_events_available_class_mask: ClassField,
    pub task_retry_period: Duration,
    pub max_tx_frag_size: usize,
    pub max_rx_frag_size: usize,
}

impl Default for MasterParams {
    fn default() -> Self {
        Self {
            response_timeout: Duration::from_secs(5),
            time_sync_mode: TimeSyncMode::None,
            disable_unsol_on_startup: true,
            ignore_restart_iin: false,
            unsol_class_mask: ClassField::ALL_EVENT_CLASSES,
            startup_integrity_class_mask: ClassField::ALL_CLASSES,
            integrity_on_event_overflow_iin: true,
            event_scan_on_events_available_class_mask: ClassField::NONE,
            task_retry_period: Duration::from_secs(5),
            max_tx_frag_size: DEFAULT_MAX_APDU_SIZE,
            max_rx_frag_size: DEFAULT_MAX_APDU_SIZE,
        }
    }
}
