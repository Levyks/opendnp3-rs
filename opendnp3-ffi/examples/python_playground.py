"""
Real end-to-end Python asyncio TCP playground for opendnp3-ffi (BoltFFI 0.32.1).

Zero Tokio in Rust, zero third-party deps in Python:
- Python owns the TCP server (asyncio.start_server) and client (asyncio.open_connection).
- Python pumps bytes between asyncio sockets and `BytePipe`.
- Rust's opendnp3 state machines are polled directly by Python's asyncio event loop.

Run after `boltffi pack python` in `opendnp3-ffi/`:
    python opendnp3-ffi/examples/python_playground.py
"""

from __future__ import annotations

import asyncio
import sys
from pathlib import Path

# Local layout: generated wheel build next to the handwritten adapter.
_FFI = Path(__file__).resolve().parent.parent
_DIST_LIB = _FFI / "dist" / "python" / "build"
for _candidate in _DIST_LIB.glob("lib.*"):
    if (_candidate / "opendnp3").is_dir():
        sys.path.insert(0, str(_candidate))
        break
sys.path.insert(0, str(_FFI.parent / "bindings" / "python"))

import opendnp3 as dnp3
import opendnp3_tcp as tcp


class PlaygroundSoeHandler:
    """Implements Rust's `SoeHandler` trait for Master measurement callbacks."""

    def __init__(self) -> None:
        self.binaries: list[dnp3.Binary] = []
        self.analogs: list[dnp3.Analog] = []

    def on_binary(self, values: list[dnp3.Binary]) -> None:
        for v in values:
            print(f"  [Master SOE] Binary[{v.index}] = {v.value} (flags=0x{v.flags:02x})")
        self.binaries.extend(values)

    def on_double_bit(self, values: list[dnp3.DoubleBitBinary]) -> None:
        pass

    def on_binary_output_status(self, values: list[dnp3.BinaryOutputStatus]) -> None:
        pass

    def on_counter(self, values: list[dnp3.Counter]) -> None:
        pass

    def on_frozen_counter(self, values: list[dnp3.FrozenCounter]) -> None:
        pass

    def on_analog(self, values: list[dnp3.Analog]) -> None:
        for v in values:
            print(f"  [Master SOE] Analog[{v.index}] = {v.value} (flags=0x{v.flags:02x})")
        self.analogs.extend(values)

    def on_analog_output_status(self, values: list[dnp3.AnalogOutputStatus]) -> None:
        pass

    def on_octet_string(self, values: list[dnp3.OctetString]) -> None:
        pass


class PlaygroundCommandHandler:
    """Implements Rust's `CommandHandler` trait for Outstation control callbacks."""

    def __init__(self) -> None:
        self.selected: list[int] = []
        self.operated: list[tuple[int, dnp3.OperateType]] = []

    def select_crob(self, command: dnp3.Crob, index: int) -> dnp3.CommandStatus:
        print(f"  [Outstation CMD] SELECT CROB index={index} op={command.op_type.name}")
        self.selected.append(index)
        return dnp3.CommandStatus.SUCCESS

    def operate_crob(
        self, command: dnp3.Crob, index: int, op_type: dnp3.OperateType
    ) -> dnp3.CommandStatus:
        print(
            f"  [Outstation CMD] OPERATE CROB index={index} op={command.op_type.name} mode={op_type.name}"
        )
        self.operated.append((index, op_type))
        return dnp3.CommandStatus.SUCCESS

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


async def main() -> None:
    all_classes = dnp3.ClassField(class0=True, class1=True, class2=True, class3=True)
    all_events = dnp3.ClassField(class0=False, class1=True, class2=True, class3=True)
    no_classes = dnp3.ClassField(class0=False, class1=False, class2=False, class3=False)
    invalid_time = dnp3.DnpTime(value_ms=0, quality=dnp3.TimestampQuality.INVALID)

    # 1. Configure and create Outstation
    db = dnp3.DatabaseConfig.all_types(3)
    cmd_handler = PlaygroundCommandHandler()
    outstation = dnp3.Outstation(
        link_cfg=dnp3.LinkConfig(
            is_master=False,
            local_addr=10,
            remote_addr=1,
            timeout_ms=1000,
            keep_alive_timeout_ms=60_000,
            respond_to_any_source=False,
        ),
        params=dnp3.OutstationParams(
            max_tx_frag_size=2048,
            max_rx_frag_size=2048,
            allow_unsolicited=False,
            sol_confirm_timeout_ms=5000,
            unsol_confirm_timeout_ms=5000,
            unsol_retry_delay_ms=5000,
            select_timeout_ms=10_000,
            max_controls_per_request=16,
            unsol_class_mask=all_events,
            respond_to_any_master=False,
            event_buffer_size_per_type=10,
        ),
        database=db,
        cmd_handler=cmd_handler,
    )

    outstation.update_binary(
        dnp3.Binary(index=0, value=True, flags=0x81, time=invalid_time),
        dnp3.EventMode.SUPPRESS,
    )
    outstation.update_analog(
        dnp3.Analog(index=2, value=230.0, flags=0x01, time=invalid_time),
        dnp3.EventMode.SUPPRESS,
    )

    server = await tcp.listen("127.0.0.1", 0, outstation.run_with_pipe)
    port = server.sockets[0].getsockname()[1]
    print(f"Outstation TCP server listening on 127.0.0.1:{port}")

    # 2. Configure and create Master
    soe = PlaygroundSoeHandler()
    master = dnp3.Master(
        link_cfg=dnp3.LinkConfig(
            is_master=True,
            local_addr=1,
            remote_addr=10,
            timeout_ms=1000,
            keep_alive_timeout_ms=60_000,
            respond_to_any_source=False,
        ),
        params=dnp3.MasterParams(
            response_timeout_ms=5000,
            time_sync_mode=dnp3.TimeSyncMode.NONE,
            disable_unsol_on_startup=False,
            ignore_restart_iin=False,
            unsol_class_mask=no_classes,
            startup_integrity_class_mask=all_classes,
            integrity_on_event_overflow_iin=True,
            event_scan_on_events_available_class_mask=no_classes,
            max_tx_frag_size=2048,
            max_rx_frag_size=2048,
        ),
        soe=soe,
    )

    master_task, writer = await tcp.connect("127.0.0.1", port, master.run_with_pipe)

    try:
        # 3. Wait for startup integrity poll
        print("Waiting for startup integrity poll...")
        while True:
            ev = await master.next_event()
            if (
                isinstance(ev, dnp3.MasterEventTaskComplete)
                and ev.task_type == dnp3.MasterTaskType.STARTUP_INTEGRITY_POLL
            ):
                print(f"Startup integrity poll finished: {ev.result.name}")
                assert ev.result == dnp3.TaskCompletion.SUCCESS
                break

        # 4. Send Select-Before-Operate CROB command
        print("Sending SBO CROB command to index 1...")
        cmds = dnp3.CommandSet()
        cmds.add_crob(
            1,
            dnp3.Crob(
                op_type=dnp3.OperationType.PULSE_ON,
                tcc=dnp3.TripCloseCode.CLOSE,
                clear=False,
                count=1,
                on_time_ms=100,
                off_time_ms=100,
                status=dnp3.CommandStatus.SUCCESS,
            ),
        )
        master.select_and_operate(cmds)

        while True:
            ev = await master.next_event()
            if isinstance(ev, dnp3.MasterEventCommandComplete):
                print(
                    f"Command complete: summary={ev.result.summary.name}, "
                    f"points={[(p.index, p.status.name) for p in ev.result.points]}"
                )
                assert ev.result.summary == dnp3.TaskCompletion.SUCCESS
                assert ev.result.points[0].status == dnp3.CommandStatus.SUCCESS
                break

        # 5. Live point update + on-demand scan
        print("Updating Analog[2] -> 241 and scanning...")
        outstation.update_analog(
            dnp3.Analog(index=2, value=241.0, flags=0x01, time=invalid_time),
            dnp3.EventMode.FORCE,
        )
        master.scan_classes(all_classes)
        while True:
            ev = await master.next_event()
            if isinstance(ev, dnp3.MasterEventTaskComplete):
                print(f"On-demand scan finished: {ev.task_type.name} -> {ev.result.name}")
                assert ev.result == dnp3.TaskCompletion.SUCCESS
                break

        # Verify all assertions
        assert any(b.index == 0 and b.value is True for b in soe.binaries)
        assert any(a.index == 2 and abs(a.value - 230.0) < 1e-6 for a in soe.analogs)
        assert any(a.index == 2 and abs(a.value - 241.0) < 1e-6 for a in soe.analogs)
        assert cmd_handler.selected == [1]
        assert cmd_handler.operated == [(1, dnp3.OperateType.SELECT_BEFORE_OPERATE)]
        print("\nALL PYTHON ASYNCIO DNP3 CHECKS PASSED!")
    finally:
        master.shutdown()
        outstation.shutdown()
        await master_task
        writer.close()
        await writer.wait_closed()
        server.close()
        await server.wait_closed()


if __name__ == "__main__":
    asyncio.run(main())
