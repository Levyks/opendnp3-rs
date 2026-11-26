//! Tokio transport integration (TCP, TLS, serial) for [`opendnp3`].

pub mod clock;
pub mod master;
pub mod outstation;
#[cfg(feature = "serial")]
pub mod serial;
pub mod stream;
#[cfg(feature = "tls")]
pub mod tls;

pub use clock::TokioClock;
pub use master::{
    RetryStrategy, run_master_session, spawn_tcp_master, spawn_tcp_master_reconnecting,
    spawn_tcp_master_server,
};
pub use opendnp3::driver::{
    AsyncStream, MasterCommand, MasterHandle, OutstationCommand, OutstationHandle, StreamError,
    UnboundedReceiver, UnboundedSender,
};
pub use outstation::{run_outstation_session, spawn_tcp_outstation};
#[cfg(feature = "serial")]
pub use serial::open_serial_stream;
pub use stream::TokioStream;
#[cfg(feature = "tls")]
pub use tls::{spawn_tls_master, spawn_tls_outstation};
