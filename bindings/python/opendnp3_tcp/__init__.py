"""asyncio TCP sessions for the generated ``opendnp3`` bindings."""

from __future__ import annotations

import asyncio
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
) -> asyncio.Server:
    """Accept TCP clients and run one ``BytePipe`` session per connection."""

    async def handle(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        pipe = dnp3.BytePipe()
        try:
            await run_over_tcp(session_factory(pipe), pipe, reader, writer)
        finally:
            writer.close()
            await writer.wait_closed()

    return await asyncio.start_server(handle, host, port)


async def connect(
    host: str,
    port: int,
    session_factory: SessionFactory[T],
) -> tuple[asyncio.Task[T], asyncio.StreamWriter]:
    """Open one TCP client and run ``session_factory`` on its pipe."""

    reader, writer = await asyncio.open_connection(host, port)
    pipe = dnp3.BytePipe()
    task = asyncio.create_task(run_over_tcp(session_factory(pipe), pipe, reader, writer))
    return task, writer


__all__ = ["connect", "listen", "run_over_tcp"]
