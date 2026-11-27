"""asyncio TCP sessions for the generated ``opendnp3`` bindings."""

from __future__ import annotations

import asyncio
import socket
import ssl as _ssl
from collections.abc import Awaitable, Callable
from typing import TypeVar

import opendnp3 as dnp3

T = TypeVar("T")

SessionFactory = Callable[[dnp3.BytePipe], Awaitable[T]]


async def run_over_tcp(
    session: Awaitable[T],
    pipe: dnp3.BytePipe,
    reader: asyncio.StreamReader,
    writer: asyncio.StreamWriter,
) -> T:
    """Pump one asyncio stream through ``pipe`` until ``session`` finishes."""
    sock = writer.get_extra_info("socket")
    if sock is not None:
        try:
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        except OSError:
            pass

    async def pump_rx() -> None:
        try:
            while True:
                data = await reader.read(4096)
                if not data:
                    break
                if not pipe.push_rx(data):
                    break
        except (OSError, asyncio.CancelledError):
            pass
        finally:
            pipe.close_pipe()

    async def pump_tx() -> None:
        try:
            while (data := await pipe.next_tx()) is not None:
                writer.write(data)
                await writer.drain()
        except (OSError, asyncio.CancelledError):
            pass

    rx_task = asyncio.create_task(pump_rx())
    tx_task = asyncio.create_task(pump_tx())
    try:
        return await session
    finally:
        pipe.close_pipe()
        rx_task.cancel()
        await asyncio.gather(rx_task, tx_task, return_exceptions=True)


async def listen(
    host: str,
    port: int,
    session_factory: SessionFactory[T],
    *,
    ssl: _ssl.SSLContext | None = None,
) -> asyncio.Server:
    """Accept TCP (or TLS when ``ssl`` is provided) clients and run one ``BytePipe`` session per connection."""

    async def handle(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        pipe = dnp3.BytePipe()
        try:
            await run_over_tcp(session_factory(pipe), pipe, reader, writer)
        finally:
            writer.close()
            await writer.wait_closed()

    return await asyncio.start_server(handle, host, port, ssl=ssl)


async def connect(
    host: str,
    port: int,
    session_factory: SessionFactory[T],
    *,
    ssl: _ssl.SSLContext | bool | None = None,
    server_hostname: str | None = None,
) -> tuple[asyncio.Task[T], asyncio.StreamWriter]:
    """Open one TCP (or TLS when ``ssl`` is provided) client and run ``session_factory`` on its pipe."""

    reader, writer = await asyncio.open_connection(
        host,
        port,
        ssl=ssl,
        server_hostname=server_hostname,
    )
    pipe = dnp3.BytePipe()
    task = asyncio.create_task(run_over_tcp(session_factory(pipe), pipe, reader, writer))
    return task, writer


def make_client_ssl_context(
    *,
    cafile: str | None = None,
    certfile: str | None = None,
    keyfile: str | None = None,
    check_hostname: bool = True,
) -> _ssl.SSLContext:
    """Create a TLS/mTLS client ``ssl.SSLContext`` (TLS 1.2+ minimum)."""
    ctx = _ssl.create_default_context(_ssl.Purpose.SERVER_AUTH, cafile=cafile)
    ctx.minimum_version = _ssl.TLSVersion.TLSv1_2
    ctx.check_hostname = check_hostname
    if certfile is not None:
        ctx.load_cert_chain(certfile=certfile, keyfile=keyfile)
    return ctx


def make_server_ssl_context(
    certfile: str,
    keyfile: str | None = None,
    *,
    cafile: str | None = None,
    require_client_cert: bool = False,
) -> _ssl.SSLContext:
    """Create a TLS/mTLS server ``ssl.SSLContext`` (TLS 1.2+ minimum)."""
    ctx = _ssl.create_default_context(_ssl.Purpose.CLIENT_AUTH, cafile=cafile)
    ctx.minimum_version = _ssl.TLSVersion.TLSv1_2
    ctx.load_cert_chain(certfile=certfile, keyfile=keyfile)
    if require_client_cert:
        ctx.verify_mode = _ssl.CERT_REQUIRED
    return ctx


class SoeHandlerBase:
    """No-op base class for ``SoeHandler``. Override only the measurement types you care about."""

    def on_binary(self, values: list[dnp3.Binary]) -> None:
        pass

    def on_double_bit(self, values: list[dnp3.DoubleBitBinary]) -> None:
        pass

    def on_binary_output_status(self, values: list[dnp3.BinaryOutputStatus]) -> None:
        pass

    def on_counter(self, values: list[dnp3.Counter]) -> None:
        pass

    def on_frozen_counter(self, values: list[dnp3.FrozenCounter]) -> None:
        pass

    def on_analog(self, values: list[dnp3.Analog]) -> None:
        pass

    def on_analog_output_status(self, values: list[dnp3.AnalogOutputStatus]) -> None:
        pass

    def on_octet_string(self, values: list[dnp3.OctetString]) -> None:
        pass


class CommandHandlerBase:
    """Default-reject base class for ``CommandHandler``. Override only the commands you support."""

    def select_crob(self, command: dnp3.Crob, index: int) -> dnp3.CommandStatus:
        return dnp3.CommandStatus.NOT_SUPPORTED

    def operate_crob(
        self, command: dnp3.Crob, index: int, op_type: dnp3.OperateType
    ) -> dnp3.CommandStatus:
        return dnp3.CommandStatus.NOT_SUPPORTED

    def select_ao_i32(self, value: int, index: int) -> dnp3.CommandStatus:
        return dnp3.CommandStatus.NOT_SUPPORTED

    def operate_ao_i32(
        self, value: int, index: int, op_type: dnp3.OperateType
    ) -> dnp3.CommandStatus:
        return dnp3.CommandStatus.NOT_SUPPORTED

    def select_ao_i16(self, value: int, index: int) -> dnp3.CommandStatus:
        return dnp3.CommandStatus.NOT_SUPPORTED

    def operate_ao_i16(
        self, value: int, index: int, op_type: dnp3.OperateType
    ) -> dnp3.CommandStatus:
        return dnp3.CommandStatus.NOT_SUPPORTED

    def select_ao_f32(self, value: float, index: int) -> dnp3.CommandStatus:
        return dnp3.CommandStatus.NOT_SUPPORTED

    def operate_ao_f32(
        self, value: float, index: int, op_type: dnp3.OperateType
    ) -> dnp3.CommandStatus:
        return dnp3.CommandStatus.NOT_SUPPORTED

    def select_ao_f64(self, value: float, index: int) -> dnp3.CommandStatus:
        return dnp3.CommandStatus.NOT_SUPPORTED

    def operate_ao_f64(
        self, value: float, index: int, op_type: dnp3.OperateType
    ) -> dnp3.CommandStatus:
        return dnp3.CommandStatus.NOT_SUPPORTED


__all__ = [
    "CommandHandlerBase",
    "SoeHandlerBase",
    "connect",
    "listen",
    "make_client_ssl_context",
    "make_server_ssl_context",
    "run_over_tcp",
]
