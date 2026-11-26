//! DNP3 Master implementation: configuration, SOE/application handlers,
//! command builder, and the runtime-agnostic [`Master`] state machine.

pub mod command;
pub mod config;
pub mod context;
pub mod handler;

pub use command::{CommandHeader, CommandSet};
pub use config::MasterParams;
pub use context::{Master, MasterEvent};
pub use handler::{
    CommandPointResult, CommandResult, DefaultMasterApplication, MasterApplication, NullSoeHandler,
    ResponseInfo, SoeHandler,
};
