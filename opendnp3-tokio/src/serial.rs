//! Serial (RS-232 / RS-485) transport helpers backed by `tokio-serial`.

use tokio_serial::{SerialPortBuilderExt, SerialStream};

use crate::stream::TokioStream;

/// Open a serial port at `path` (e.g. `COM3` or `/dev/ttyUSB0`) and wrap it in
/// a [`TokioStream`] ready for [`opendnp3::driver::run_master_session`] or
/// [`opendnp3::driver::run_outstation_session`].
pub fn open_serial_stream(
    path: impl Into<String>,
    baud_rate: u32,
) -> Result<TokioStream<SerialStream>, tokio_serial::Error> {
    let port = tokio_serial::new(path.into(), baud_rate).open_native_async()?;
    Ok(TokioStream::new(port))
}
