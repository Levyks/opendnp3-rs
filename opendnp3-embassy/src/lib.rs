//! `#![no_std]` Embassy adapter for [`opendnp3`] built on `embedded-io-async`
//! and `embassy-time`.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration as EmbassyDuration, Instant as EmbassyInstant, Timer};
use embedded_io_async::{Read, Write};
use opendnp3::clock::MonotonicTime;
use opendnp3::driver::{
    LocalAsyncStream, MasterCommand, OutstationCommand, StreamError, UnboundedReceiver,
    UnboundedSender, run_master_session_local, run_outstation_session_local,
};
use opendnp3::link::LinkConfig;
use opendnp3::master::{Master, MasterApplication, MasterEvent, SoeHandler};
use opendnp3::outstation::{CommandHandler, Outstation, OutstationApplication};

/// [`LocalAsyncStream`] adapter wrapping any split `embedded_io_async::Read`
/// and `embedded_io_async::Write` pair (e.g. `embassy_net::tcp::TcpSocket::split()`,
/// UART/RS-485 `split()`, etc.).
pub struct EmbassyStream<R, W> {
    reader: Mutex<NoopRawMutex, R>,
    writer: Mutex<NoopRawMutex, W>,
}

impl<R: Read, W: Write> EmbassyStream<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader: Mutex::new(reader),
            writer: Mutex::new(writer),
        }
    }
}

impl<R: Read, W: Write> LocalAsyncStream for EmbassyStream<R, W> {
    async fn read(&self, max_bytes: usize) -> Result<Vec<u8>, StreamError> {
        let mut buf = vec![0u8; max_bytes];
        let mut reader = self.reader.lock().await;
        match reader.read(&mut buf).await {
            Ok(0) => Err(StreamError::Closed),
            Ok(n) => {
                buf.truncate(n);
                Ok(buf)
            }
            Err(e) => Err(StreamError::Io(format!("{e:?}"))),
        }
    }

    async fn write(&self, data: &[u8]) -> Result<(), StreamError> {
        let mut writer = self.writer.lock().await;
        writer
            .write_all(data)
            .await
            .map_err(|e| StreamError::Io(format!("{e:?}")))?;
        writer
            .flush()
            .await
            .map_err(|e| StreamError::Io(format!("{e:?}")))
    }

    async fn sleep(&self, duration: Duration) {
        let ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        Timer::after(EmbassyDuration::from_millis(ms)).await;
    }

    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(EmbassyInstant::now().as_millis())
    }
}

/// Drive a [`Master`] session over an [`EmbassyStream`] (`#![no_std]`).
pub async fn run_embassy_master<R, W, H, A>(
    stream: &EmbassyStream<R, W>,
    link_cfg: LinkConfig,
    master: &mut Master,
    soe: &mut H,
    app: &mut A,
    cmd_rx: &UnboundedReceiver<MasterCommand>,
    ev_tx: &UnboundedSender<MasterEvent>,
) -> bool
where
    R: Read,
    W: Write,
    H: SoeHandler,
    A: MasterApplication,
{
    run_master_session_local(stream, link_cfg, master, soe, app, cmd_rx, ev_tx).await
}

/// Drive an [`Outstation`] session over an [`EmbassyStream`] (`#![no_std]`).
pub async fn run_embassy_outstation<R, W, C, A>(
    stream: &EmbassyStream<R, W>,
    link_cfg: LinkConfig,
    outstation: &mut Outstation,
    cmd_handler: &mut C,
    app: &mut A,
    cmd_rx: &UnboundedReceiver<OutstationCommand>,
) -> bool
where
    R: Read,
    W: Write,
    C: CommandHandler,
    A: OutstationApplication,
{
    run_outstation_session_local(stream, link_cfg, outstation, cmd_handler, app, cmd_rx).await
}
